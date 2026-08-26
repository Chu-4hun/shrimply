from dataclasses import dataclass

import torch
from torch import nn
from torch.nn import functional as functional
from transformers import GPT2Config, GPT2Model, LogitsProcessorList
from transformers import TypicalLogitsWarper

from .conformer import ConformerEncoder
from .gpt2 import GPT2InferenceModel, NullPositionEmbeddings
from .perceiver import PerceiverResampler


@dataclass(frozen=True)
class ConditioningConfig:
    output_size: int
    linear_units: int
    attention_heads: int
    num_blocks: int
    input_layer: str
    perceiver_multiplier: int


@dataclass(frozen=True)
class GenerationOptions:
    do_sample: bool
    top_p: float
    top_k: int
    temperature: float
    length_penalty: float
    num_beams: int
    repetition_penalty: float
    maximum_tokens: int
    typical_sampling: bool = False
    typical_mass: float = 0.9


class LearnedPositionEmbeddings(nn.Module):
    def __init__(self, sequence_length: int, dimension: int) -> None:
        super().__init__()
        self.emb = nn.Embedding(sequence_length, dimension)
        self.emb.weight.data.normal_(mean=0.0, std=0.02)

    def forward(self, inputs: torch.Tensor) -> torch.Tensor:
        positions = torch.arange(inputs.shape[1], device=inputs.device)
        return self.emb(positions)

    def get_fixed_embedding(
        self, index: int, device: torch.device
    ) -> torch.Tensor:
        return self.emb(torch.tensor([index], device=device)).unsqueeze(0)


class UnifiedVoice(nn.Module):
    def __init__(
        self,
        *,
        layers: int,
        model_dim: int,
        heads: int,
        max_text_tokens: int,
        max_mel_tokens: int,
        mel_length_compression: int,
        number_text_tokens: int,
        start_text_token: int,
        stop_text_token: int,
        number_mel_codes: int,
        start_mel_token: int,
        stop_mel_token: int,
        condition_module: ConditioningConfig,
        emo_condition_module: ConditioningConfig,
    ) -> None:
        super().__init__()
        self.number_text_tokens = number_text_tokens
        self.start_text_token = start_text_token
        self.stop_text_token = stop_text_token
        self.number_mel_codes = number_mel_codes
        self.start_mel_token = start_mel_token
        self.stop_mel_token = stop_mel_token
        self.layers = layers
        self.heads = heads
        self.max_mel_tokens = max_mel_tokens
        self.max_text_tokens = max_text_tokens
        self.model_dim = model_dim
        self.max_conditioning_inputs = 1
        self.mel_length_compression = mel_length_compression
        self.condition_type = "conformer_perceiver"
        self.cond_num = 32
        self.cond_mask_pad = nn.ConstantPad1d((self.cond_num, 0), True)
        self.emo_cond_mask_pad = nn.ConstantPad1d((1, 0), True)

        self.conditioning_encoder = ConformerEncoder(
            1024,
            condition_module.output_size,
            condition_module.attention_heads,
            condition_module.linear_units,
            condition_module.num_blocks,
            condition_module.input_layer,
        )
        self.perceiver_encoder = PerceiverResampler(
            model_dim,
            context_dimension=condition_module.output_size,
            feed_forward_multiplier=condition_module.perceiver_multiplier,
            heads=condition_module.attention_heads,
            latent_count=self.cond_num,
        )
        self.emo_conditioning_encoder = ConformerEncoder(
            1024,
            emo_condition_module.output_size,
            emo_condition_module.attention_heads,
            emo_condition_module.linear_units,
            emo_condition_module.num_blocks,
            emo_condition_module.input_layer,
        )
        self.emo_perceiver_encoder = PerceiverResampler(
            1024,
            context_dimension=emo_condition_module.output_size,
            feed_forward_multiplier=emo_condition_module.perceiver_multiplier,
            heads=emo_condition_module.attention_heads,
            latent_count=1,
        )

        self.text_embedding = nn.Embedding(number_text_tokens + 1, model_dim)
        self.emo_layer = nn.Linear(model_dim, model_dim)
        self.emovec_layer = nn.Linear(1024, model_dim)
        self.mel_embedding = nn.Embedding(number_mel_codes, model_dim)

        transformer_config = GPT2Config(
            vocab_size=256,
            n_positions=max_mel_tokens + max_text_tokens + 5,
            n_embd=model_dim,
            n_layer=layers,
            n_head=heads,
            use_cache=False,
            bos_token_id=None,
            eos_token_id=None,
        )
        self.gpt = GPT2Model(transformer_config)
        self.gpt.gradient_checkpointing_enable()
        setattr(self.gpt, "wpe", NullPositionEmbeddings(model_dim))
        del self.gpt.wte
        self.mel_pos_embedding = LearnedPositionEmbeddings(
            max_mel_tokens + 3, model_dim
        )
        self.text_pos_embedding = LearnedPositionEmbeddings(
            max_text_tokens + 2, model_dim
        )
        self.mel_layer_pos_embedding = None
        self.text_layer_pos_embedding = None
        self.mel_solo_embedding = 0
        self.text_solo_embedding = 0
        self.final_norm = nn.LayerNorm(model_dim)
        self.text_head = nn.Linear(model_dim, number_text_tokens + 1)
        self.mel_head = nn.Linear(model_dim, number_mel_codes)
        self.speed_emb = nn.Embedding(2, model_dim)
        self.speed_emb.weight.data.zero_()
        self.text_embedding.weight.data.normal_(mean=0.0, std=0.02)
        self.mel_embedding.weight.data.normal_(mean=0.0, std=0.02)
        self.inference_model: GPT2InferenceModel | None = None

    def prepare_for_inference(self) -> None:
        sequence_length = self.max_mel_tokens + self.max_text_tokens + 2
        config = GPT2Config(
            vocab_size=self.number_mel_codes,
            n_positions=sequence_length,
            n_embd=self.model_dim,
            n_layer=self.layers,
            n_head=self.heads,
            use_cache=True,
            bos_token_id=self.start_mel_token,
            eos_token_id=self.stop_mel_token,
            pad_token_id=self.stop_mel_token,
        )
        self.inference_model = GPT2InferenceModel(
            config,
            self.gpt,
            self.mel_pos_embedding,
            self.mel_embedding,
            self.final_norm,
            self.mel_head,
            kv_cache=True,
        ).eval()
        self.gpt.wte = self.mel_embedding

    def set_padding(
        self, tokens: torch.Tensor, lengths: torch.Tensor, stop_token: int
    ) -> torch.Tensor:
        for batch in range(len(lengths)):
            end = int(lengths[batch])
            if end < tokens.shape[-1]:
                tokens[batch, end:] = stop_token
        return tokens

    def get_conditioning(
        self, inputs: torch.Tensor, lengths: torch.Tensor
    ) -> torch.Tensor:
        encoded, mask = self.conditioning_encoder(inputs.transpose(1, 2), lengths)
        padded_mask = self.cond_mask_pad(mask.squeeze(1))
        return self.perceiver_encoder(encoded, padded_mask)

    def get_emo_conditioning(
        self, inputs: torch.Tensor, lengths: torch.Tensor
    ) -> torch.Tensor:
        encoded, mask = self.emo_conditioning_encoder(
            inputs.transpose(1, 2), lengths
        )
        padded_mask = self.emo_cond_mask_pad(mask.squeeze(1))
        return self.emo_perceiver_encoder(encoded, padded_mask).squeeze(1)

    def emotion_vector(
        self, conditioning: torch.Tensor, lengths: torch.Tensor
    ) -> torch.Tensor:
        encoded = self.get_emo_conditioning(conditioning.transpose(1, 2), lengths)
        return self.emo_layer(self.emovec_layer(encoded))

    def merge_emotion_vectors(
        self,
        speaker_conditioning: torch.Tensor,
        emotion_conditioning: torch.Tensor,
        speaker_lengths: torch.Tensor,
        emotion_lengths: torch.Tensor,
        strength: float = 1.0,
    ) -> torch.Tensor:
        emotion = self.emotion_vector(emotion_conditioning, emotion_lengths)
        speaker = self.emotion_vector(speaker_conditioning, speaker_lengths)
        return speaker + strength * (emotion - speaker)

    def forward(
        self,
        speech_conditioning_latent: torch.Tensor,
        text_inputs: torch.Tensor,
        text_lengths: torch.Tensor,
        mel_codes: torch.Tensor,
        mel_code_lengths: torch.Tensor,
        emotion_conditioning_latent: torch.Tensor,
        emotion_vector: torch.Tensor | None = None,
    ) -> torch.Tensor:
        if emotion_vector is None:
            emotion_lengths = torch.full(
                (emotion_conditioning_latent.shape[0],),
                emotion_conditioning_latent.shape[1],
                device=emotion_conditioning_latent.device,
            )
            emotion_vector = self.emotion_vector(
                emotion_conditioning_latent, emotion_lengths
            )
        text_inputs = self.set_padding(
            text_inputs, text_lengths, self.stop_text_token
        )
        text_inputs = functional.pad(text_inputs, (0, 1), value=self.stop_text_token)
        mel_codes = self.set_padding(mel_codes, mel_code_lengths, self.stop_mel_token)
        mel_codes = functional.pad(mel_codes, (0, 1), value=self.stop_mel_token)
        batch = speech_conditioning_latent.shape[0]
        duration = self.speed_emb(
            torch.zeros(batch, dtype=torch.long, device=speech_conditioning_latent.device)
        )
        half_duration = self.speed_emb(
            torch.ones(batch, dtype=torch.long, device=speech_conditioning_latent.device)
        )
        conditions = torch.cat(
            (
                speech_conditioning_latent + emotion_vector.unsqueeze(1),
                half_duration.unsqueeze(1),
                duration.unsqueeze(1),
            ),
            dim=1,
        )
        text_with_start = functional.pad(
            text_inputs, (1, 0), value=self.start_text_token
        )
        mel_with_start = functional.pad(
            mel_codes, (1, 0), value=self.start_mel_token
        )
        text_embeddings = (
            self.text_embedding(text_with_start)
            + self.text_pos_embedding(text_with_start)
        )
        mel_embeddings = (
            self.mel_embedding(mel_with_start) + self.mel_pos_embedding(mel_with_start)
        )
        combined = torch.cat((conditions, text_embeddings, mel_embeddings), dim=1)
        hidden = self.final_norm(
            self.gpt(inputs_embeds=combined, return_dict=True).last_hidden_state[
                :, conditions.shape[1] :
            ]
        )
        mel_hidden = hidden[:, -mel_embeddings.shape[1] :]
        return mel_hidden[:, :-2]

    def prepare_gpt_inputs(
        self, conditions: torch.Tensor, text_inputs: torch.Tensor
    ) -> tuple[torch.Tensor, torch.Tensor, torch.Tensor]:
        batch, padded_text_length = text_inputs.shape
        target_length = conditions.shape[1] + padded_text_length + 2
        prefixes: list[torch.Tensor] = []
        masks: list[torch.Tensor] = []
        shared_condition = conditions.shape[0] == 1
        for index in range(batch):
            valid = (text_inputs[index] != self.stop_text_token) & (
                text_inputs[index] != self.start_text_token
            )
            text = functional.pad(
                text_inputs[index][valid], (1, 1), value=self.start_text_token
            )
            text[-1] = self.stop_text_token
            positions = torch.arange(text.shape[0], device=text.device)
            text_embedding = self.text_embedding(text) + self.text_pos_embedding.emb(
                positions
            )
            condition = conditions.squeeze(0) if shared_condition else conditions[index]
            parts = [condition, text_embedding]
            mask = torch.ones(target_length + 1, dtype=torch.long, device=text.device)
            padding = padded_text_length + 2 - text.shape[0]
            if padding:
                parts.insert(
                    0,
                    torch.zeros(
                        padding,
                        conditions.shape[-1],
                        dtype=text_embedding.dtype,
                        device=text.device,
                    ),
                )
                mask[:padding] = 0
            prefix = torch.cat(parts)
            if prefix.shape[0] != target_length:
                raise RuntimeError("IndexTTS 2 produced an invalid GPT prefix length")
            prefixes.append(prefix)
            masks.append(mask)
        prefix_batch = torch.stack(prefixes)
        attention_mask = torch.stack(masks)
        input_ids = torch.ones(
            (batch, prefix_batch.shape[1] + 1),
            dtype=torch.long,
            device=text_inputs.device,
        )
        input_ids[:, -1] = self.start_mel_token
        return input_ids, prefix_batch, attention_mask

    def inference_speech(
        self,
        speaker_conditioning: torch.Tensor,
        text_inputs: torch.Tensor,
        emotion_conditioning: torch.Tensor,
        speaker_lengths: torch.Tensor,
        emotion_lengths: torch.Tensor,
        emotion_vector: torch.Tensor | None,
        options: GenerationOptions,
    ) -> tuple[torch.Tensor, torch.Tensor]:
        if self.inference_model is None:
            raise RuntimeError("IndexTTS 2 GPT was not prepared for inference")
        speaker_latent = self.get_conditioning(
            speaker_conditioning.transpose(1, 2), speaker_lengths
        )
        if emotion_vector is None:
            emotion_vector = self.emotion_vector(
                emotion_conditioning, emotion_lengths
            )
        batch = text_inputs.shape[0]
        duration = self.speed_emb(
            torch.zeros(batch, dtype=torch.long, device=text_inputs.device)
        )
        half_duration = self.speed_emb(
            torch.ones(batch, dtype=torch.long, device=text_inputs.device)
        )
        conditions = torch.cat(
            (
                speaker_latent + emotion_vector.unsqueeze(1),
                half_duration.unsqueeze(1),
                duration.unsqueeze(1),
            ),
            dim=1,
        )
        input_ids, prefix, attention_mask = self.prepare_gpt_inputs(
            conditions, text_inputs
        )
        self.inference_model.store_mel_emb(prefix)
        processors = LogitsProcessorList()
        if options.typical_sampling:
            processors.append(
                TypicalLogitsWarper(
                    mass=options.typical_mass,
                    min_tokens_to_keep=2 if options.num_beams > 1 else 1,
                )
            )
        generate = getattr(self.inference_model, "generate", None)
        if not callable(generate):
            raise TypeError("IndexTTS inference model does not support generation")
        output = generate(
            input_ids,
            bos_token_id=self.start_mel_token,
            pad_token_id=self.stop_mel_token,
            eos_token_id=self.stop_mel_token,
            attention_mask=attention_mask,
            max_length=input_ids.shape[1] + options.maximum_tokens,
            logits_processor=processors,
            do_sample=options.do_sample,
            top_p=options.top_p,
            top_k=options.top_k,
            temperature=options.temperature,
            length_penalty=options.length_penalty,
            num_beams=options.num_beams,
            repetition_penalty=options.repetition_penalty,
        )
        if not isinstance(output, torch.Tensor):
            raise TypeError("IndexTTS inference model returned invalid token IDs")
        return output[:, input_ids.shape[1] :], speaker_latent

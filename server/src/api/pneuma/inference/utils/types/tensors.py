import torch

type FeatureTensor = torch.Tensor
type SSLFeatureTensor = torch.Tensor
type HubertBaseFeatureTensor = torch.Tensor
type HubertLargeFeatureTensor = torch.Tensor
type ConditioningTensor = torch.Tensor
type AudioTensor = torch.Tensor
type PaddingMaskTensor = torch.Tensor
type WaveformTensor = torch.Tensor
type ScalarLengthTensor = torch.Tensor

__all__ = [
    "AudioTensor",
    "ConditioningTensor",
    "FeatureTensor",
    "HubertBaseFeatureTensor",
    "HubertLargeFeatureTensor",
    "PaddingMaskTensor",
    "ScalarLengthTensor",
    "SSLFeatureTensor",
    "WaveformTensor",
]


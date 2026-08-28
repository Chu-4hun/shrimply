use crate::Color;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LayerBlendMode {
    PassThrough,
    #[default]
    Normal,
    Dissolve,
    Darken,
    Multiply,
    ColorBurn,
    LinearBurn,
    DarkerColor,
    Lighten,
    Screen,
    ColorDodge,
    Add,
    LighterColor,
    Overlay,
    SoftLight,
    HardLight,
    VividLight,
    LinearLight,
    PinLight,
    HardMix,
    Difference,
    Exclusion,
    Subtract,
    Divide,
    Reflect,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl Color<f32> {
    pub fn blend_rgb<const CLAMP: bool>(self, destination: Self, mode: LayerBlendMode) -> Self {
        let mut color = match mode {
            LayerBlendMode::Hue => self
                .with_saturation(destination.saturation())
                .with_luminosity(destination.luminosity()),
            LayerBlendMode::Saturation => destination
                .with_saturation(self.saturation())
                .with_luminosity(destination.luminosity()),
            LayerBlendMode::Color => self.with_luminosity(destination.luminosity()),
            LayerBlendMode::Luminosity => destination.with_luminosity(self.luminosity()),
            LayerBlendMode::DarkerColor => {
                if self.luminosity() < destination.luminosity() {
                    self
                } else {
                    destination
                }
            }
            LayerBlendMode::LighterColor => {
                if self.luminosity() > destination.luminosity() {
                    self
                } else {
                    destination
                }
            }
            _ => Self::new(
                blend_channel(self.r, destination.r, mode),
                blend_channel(self.g, destination.g, mode),
                blend_channel(self.b, destination.b, mode),
                self.a,
            ),
        };
        if CLAMP {
            color.r = color.r.clamp(0.0, 1.0);
            color.g = color.g.clamp(0.0, 1.0);
            color.b = color.b.clamp(0.0, 1.0);
        }
        color
    }

    pub fn blend_over<const CLAMP: bool>(
        self,
        destination: Self,
        mode: LayerBlendMode,
        source_alpha: f32,
    ) -> Self {
        let source_alpha = source_alpha.clamp(0.0, 1.0);
        let output_alpha = source_alpha + destination.a * (1.0 - source_alpha);
        if output_alpha <= f32::EPSILON {
            return Self::TRANSPARENT;
        }
        let blended = self.blend_rgb::<CLAMP>(destination, mode);
        let channel = |source: f32, destination_channel: f32, blended_channel: f32| {
            ((1.0 - destination.a) * source_alpha * source
                + (1.0 - source_alpha) * destination.a * destination_channel
                + source_alpha * destination.a * blended_channel)
                / output_alpha
        };
        Self::new(
            channel(self.r, destination.r, blended.r),
            channel(self.g, destination.g, blended.g),
            channel(self.b, destination.b, blended.b),
            output_alpha,
        )
    }

    #[inline(always)]
    fn luminosity(self) -> f32 {
        0.3 * self.r + 0.59 * self.g + 0.11 * self.b
    }

    #[inline(always)]
    fn saturation(self) -> f32 {
        self.r.max(self.g).max(self.b) - self.r.min(self.g).min(self.b)
    }

    #[inline(always)]
    fn with_luminosity(mut self, luminosity: f32) -> Self {
        let delta = luminosity - self.luminosity();
        self.r += delta;
        self.g += delta;
        self.b += delta;
        self.clip_rgb()
    }

    #[inline(always)]
    fn clip_rgb(mut self) -> Self {
        let luminosity = self.luminosity();
        let minimum = self.r.min(self.g).min(self.b);
        let maximum = self.r.max(self.g).max(self.b);
        if minimum < 0.0 {
            self.r = luminosity + (self.r - luminosity) * luminosity / (luminosity - minimum);
            self.g = luminosity + (self.g - luminosity) * luminosity / (luminosity - minimum);
            self.b = luminosity + (self.b - luminosity) * luminosity / (luminosity - minimum);
        }
        if maximum > 1.0 {
            self.r =
                luminosity + (self.r - luminosity) * (1.0 - luminosity) / (maximum - luminosity);
            self.g =
                luminosity + (self.g - luminosity) * (1.0 - luminosity) / (maximum - luminosity);
            self.b =
                luminosity + (self.b - luminosity) * (1.0 - luminosity) / (maximum - luminosity);
        }
        self.r = self.r.clamp(0.0, 1.0);
        self.g = self.g.clamp(0.0, 1.0);
        self.b = self.b.clamp(0.0, 1.0);
        self
    }

    #[inline(always)]
    fn with_saturation(mut self, saturation: f32) -> Self {
        let channels = [self.r, self.g, self.b];
        let minimum = if channels[0] <= channels[1] && channels[0] <= channels[2] {
            0
        } else if channels[1] <= channels[2] {
            1
        } else {
            2
        };
        let maximum = if channels[0] >= channels[1] && channels[0] >= channels[2] {
            0
        } else if channels[1] >= channels[2] {
            1
        } else {
            2
        };
        if minimum == maximum {
            self.r = 0.0;
            self.g = 0.0;
            self.b = 0.0;
            return self;
        }
        let middle = 3 - minimum - maximum;
        let mut output = [0.0; 3];
        if channels[maximum] > channels[minimum] {
            output[middle] = (channels[middle] - channels[minimum]) * saturation
                / (channels[maximum] - channels[minimum]);
            output[maximum] = saturation;
        }
        self.r = output[0];
        self.g = output[1];
        self.b = output[2];
        self
    }
}

#[inline(always)]
fn blend_channel(source: f32, destination: f32, mode: LayerBlendMode) -> f32 {
    match mode {
        LayerBlendMode::PassThrough | LayerBlendMode::Normal | LayerBlendMode::Dissolve => source,
        LayerBlendMode::Darken => source.min(destination),
        LayerBlendMode::Multiply => source * destination,
        LayerBlendMode::ColorBurn => color_burn(source, destination),
        LayerBlendMode::LinearBurn => (source + destination - 1.0).max(0.0),
        LayerBlendMode::Lighten => source.max(destination),
        LayerBlendMode::Screen => source + destination - source * destination,
        LayerBlendMode::ColorDodge => color_dodge(source, destination),
        LayerBlendMode::Add => (source + destination).min(1.0),
        LayerBlendMode::Overlay => hard_light(destination, source),
        LayerBlendMode::SoftLight => {
            if source <= 0.5 {
                destination - (1.0 - 2.0 * source) * destination * (1.0 - destination)
            } else {
                let curve = if destination <= 0.25 {
                    ((16.0 * destination - 12.0) * destination + 4.0) * destination
                } else {
                    destination.sqrt()
                };
                destination + (2.0 * source - 1.0) * (curve - destination)
            }
        }
        LayerBlendMode::HardLight => hard_light(source, destination),
        LayerBlendMode::VividLight => vivid_light(source, destination),
        LayerBlendMode::LinearLight => (destination + 2.0 * source - 1.0).clamp(0.0, 1.0),
        LayerBlendMode::PinLight => {
            if source <= 0.5 {
                destination.min(source * 2.0)
            } else {
                destination.max(2.0 * source - 1.0)
            }
        }
        LayerBlendMode::HardMix => (vivid_light(source, destination) >= 0.5) as u8 as f32,
        LayerBlendMode::Difference => (destination - source).abs(),
        LayerBlendMode::Exclusion => destination + source - 2.0 * destination * source,
        LayerBlendMode::Subtract => (destination - source).max(0.0),
        LayerBlendMode::Divide => {
            if source <= 0.0 {
                1.0
            } else {
                (destination / source).min(1.0)
            }
        }
        LayerBlendMode::Reflect => {
            if source >= 1.0 {
                1.0
            } else {
                (destination * destination / (1.0 - source)).min(1.0)
            }
        }
        LayerBlendMode::DarkerColor
        | LayerBlendMode::LighterColor
        | LayerBlendMode::Hue
        | LayerBlendMode::Saturation
        | LayerBlendMode::Color
        | LayerBlendMode::Luminosity => unreachable!(),
    }
}

#[inline(always)]
fn color_burn(source: f32, destination: f32) -> f32 {
    if source <= 0.0 {
        0.0
    } else {
        1.0 - ((1.0 - destination) / source).min(1.0)
    }
}

#[inline(always)]
fn color_dodge(source: f32, destination: f32) -> f32 {
    if source >= 1.0 {
        1.0
    } else {
        (destination / (1.0 - source)).min(1.0)
    }
}

#[inline(always)]
fn vivid_light(source: f32, destination: f32) -> f32 {
    if source <= 0.5 {
        color_burn(source * 2.0, destination)
    } else {
        color_dodge((source - 0.5) * 2.0, destination)
    }
}

#[inline(always)]
fn hard_light(source: f32, destination: f32) -> f32 {
    if source <= 0.5 {
        2.0 * source * destination
    } else {
        1.0 - 2.0 * (1.0 - source) * (1.0 - destination)
    }
}

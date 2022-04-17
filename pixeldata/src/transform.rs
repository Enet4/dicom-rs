//! Private module for pixel sample value transformation functions.

use snafu::Snafu;

/// Description of a modality rescale function,
/// defined by a _rescale slope_ and _rescale intercept_.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Rescale {
    /// the rescale slope
    pub slope: f64,
    /// the rescale intercept
    pub intercept: f64,
}

impl Rescale {
    /// Create a new rescale function.
    #[inline]
    pub fn new(slope: f64, intercept: f64) -> Self {
        Rescale { slope, intercept }
    }

    /// Apply the rescale function to a value.
    #[inline]
    pub fn apply(&self, value: f64) -> f64 {
        self.slope * value + self.intercept
    }
}

/// A known DICOM Value of Interest (VOI) LUT function descriptor.
#[derive(Debug, Copy, Clone, Eq, Hash, PartialEq)]
pub enum VoiLutFunction {
    /// LINEAR
    Linear,
    /// LINEAR_EXACT
    LinearExact,
    /// SIGMOID
    Sigmoid,
}

/// Unrecognized VOI LUT function name
#[derive(Debug, Copy, Clone, PartialEq, Snafu)]
pub struct FromVoiLutFunctionError {
    _private: (),
}

impl std::convert::TryFrom<&str> for VoiLutFunction {
    type Error = FromVoiLutFunctionError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "LINEAR" => Ok(Self::Linear),
            "LINEAR_EXACT" => Ok(Self::LinearExact),
            "SIGMOID" => Ok(Self::Sigmoid),
            _ => Err(FromVoiLutFunctionError { _private: () }),
        }
    }
}

impl Default for VoiLutFunction {
    fn default() -> Self {
        VoiLutFunction::Linear
    }
}

/// The parameters of a single window level
/// for a VOI LUT transformation,
/// comprising the window center and the window width.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct WindowLevel {
    /// The _Window Width_.
    ///
    /// Should be greater than 0
    pub width: f64,
    /// The _Window Center_.
    pub center: f64,
}

/// A full description of a VOI LUT function transformation
/// based on a window level.
#[derive(Debug, PartialEq)]
pub struct WindowLevelTransform {
    voi_lut_function: VoiLutFunction,
    window_level: WindowLevel,
}

impl WindowLevelTransform {
    /// Create a new window level transformation.
    ///
    /// The width of the given `window_level` is automatically clamped
    /// if it is incompatible with the given LUT function:
    /// it muse be `>= 0` if the function is [`LinearExact`](VoiLutFunction::LinearExact),
    /// and `>= 1` in other functions.
    #[inline]
    pub fn new(voi_lut_function: VoiLutFunction, window_level: WindowLevel) -> Self {
        WindowLevelTransform {
            voi_lut_function,
            window_level: WindowLevel {
                center: window_level.center,
                width: match voi_lut_function {
                    VoiLutFunction::LinearExact => window_level.width.max(0.),
                    VoiLutFunction::Linear | VoiLutFunction::Sigmoid => window_level.width.max(1.),
                },
            },
        }
    }
    
    /// Create a new window level transformation
    /// with the `LINEAR` function.
    ///
    /// The width of the given `window_level` is automatically clamped
    /// to 1 if it is lower than 1.
    #[inline]
    pub fn linear(window_level: WindowLevel) -> Self {
        Self::new(VoiLutFunction::Linear, window_level)
    }

    /// Apply the window level transformation on a rescaled value,
    /// into a number between `0` and `y_max`.
    pub fn apply(&self, value: f64, y_max: f64) -> f64 {
        match self.voi_lut_function {
            VoiLutFunction::Linear => window_level_linear(
                value,
                self.window_level.width,
                self.window_level.center,
                y_max,
            ),
            VoiLutFunction::LinearExact => window_level_linear_exact(
                value,
                self.window_level.width,
                self.window_level.center,
                y_max,
            ),
            VoiLutFunction::Sigmoid => window_level_sigmoid(
                value,
                self.window_level.width,
                self.window_level.center,
                y_max,
            ),
        }
    }
}

fn window_level_linear(value: f64, window_width: f64, window_center: f64, y_max: f64) -> f64 {
    let width = window_width as f64;
    let center = window_center as f64;
    debug_assert!(width >= 1.);

    // C.11.2.1.2.1

    let min = center - (width - 1.) / 2.;
    let max = center - 0.5 + (width - 1.) / 2.;

    if value <= min {
        // if (x <= c - (w-1) / 2), then y = ymin
        0.
    } else if value > max {
        // else if (x > c - 0.5 + (w-1) /2), then y = ymax
        y_max
    } else {
        // else y = ((x - (c - 0.5)) / (w-1) + 0.5) * (ymax- ymin) + ymin
        ((value - (center - 0.5)) / (width - 1.) + 0.5) * y_max
    }
}

fn window_level_linear_exact(value: f64, window_width: f64, window_center: f64, y_max: f64) -> f64 {
    let width = window_width as f64;
    let center = window_center as f64;
    debug_assert!(width >= 0.);

    // C.11.2.1.3.2

    let min = center - width / 2.;
    let max = center + width / 2.;

    if value <= min {
        // if (x <= c - w/2), then y = ymin
        0.
    } else if value > max {
        // else if (x > c + w/2), then y = ymax
        y_max
    } else {
        // else y = ((x - c) / w + 0.5) * (ymax - ymin) + ymin
        ((value - center) / width + 0.5) * y_max
    }
}

fn window_level_sigmoid(value: f64, window_width: f64, window_center: f64, y_max: f64) -> f64 {
    let width = window_width as f64;
    let center = window_center as f64;
    assert!(width >= 1.);

    // C.11.2.1.3.1

    y_max / (1. + f64::exp(-4. * (value - center) / width))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Applying a common rescale function to a value
    /// gives the expected output.
    #[test]
    fn modality_lut_baseline() {
        let rescale = Rescale::new(1., -1024.);

        assert_eq!(rescale.apply(0.), -1024.);
        assert_eq!(rescale.apply(1.), -1023.);
        assert_eq!(rescale.apply(2.), -1022.);
        assert_eq!(rescale.apply(1024.), 0.);
    }

    /// Applying a linear window level
    /// as per the example described in the standard
    /// (C.11.2.1.2.1)
    /// gives us the expected outcome.
    #[test]
    fn window_level_linear_example() {
        let window_level = WindowLevel {
            width: 4096.,
            center: 2048.,
        };
        let window_level_transform = WindowLevelTransform::linear(window_level);
        let y_max = 255.;

        // x <= 0
        assert_eq!(window_level_transform.apply(-2., y_max), 0.);
        assert_eq!(window_level_transform.apply(-1., y_max), 0.);
        assert_eq!(window_level_transform.apply(0., y_max), 0.);

        // x > 4095
        assert_eq!(window_level_transform.apply(4095.5, y_max), y_max);
        assert_eq!(window_level_transform.apply(4096., y_max), y_max);
        assert_eq!(window_level_transform.apply(4097., y_max), y_max);

        // inbetween:  y = ((x - 2047.5) / 4095 + 0.5) * 255

        let x = 1024.;
        let y = window_level_transform.apply(x, y_max);
        let expected_y = ((x - 2047.5) / 4095. + 0.5) * 255.;

        assert!((y - expected_y).abs() < 1e-3);
    }

    /// Applying a linear window level gives us the expected outcome.
    #[test]
    fn window_level_linear_1() {
        let window_level = WindowLevel {
            width: 300.,
            center: 50.,
        };
        let window_level_transform = WindowLevelTransform::linear(window_level);
        let y_max = 255.;

        // x <= (50 - 150)
        let y = window_level_transform.apply(-120., y_max);
        assert_eq!(y, 0.);
        let y = window_level_transform.apply(-100.5, y_max);
        assert_eq!(y, 0.);
        let y = window_level_transform.apply(-100., y_max);
        assert_eq!(y, 0.);

        // x >= (50 + 150)
        let y = window_level_transform.apply(201., y_max);
        assert_eq!(y, 255.);
        let y = window_level_transform.apply(200., y_max);
        assert_eq!(y, 255.);

        // x inbetween
        let y = window_level_transform.apply(50., y_max);
        assert!(y > 127. && y < 129.);
    }
}

/// A single-channel 8-bit image: row-major samples carried with their dimensions.
#[derive(Clone)]
pub struct Gray {
    data: Vec<u8>,
    width: usize,
    height: usize,
}

impl Gray {
    /// Panics unless `data.len() == width * height`.
    pub fn from_vec(data: Vec<u8>, width: usize, height: usize) -> Self {
        assert_eq!(
            data.len(),
            width * height,
            "{width}x{height} needs {} samples, got {}",
            width * height,
            data.len()
        );
        Self {
            data,
            width,
            height,
        }
    }

    /// Interleaved 8-bit RGB, flattened with the paper's weights:
    ///
    /// ```text
    /// grey = (54*red + 183*green + 19*blue) / 256
    /// ```
    ///
    /// They sum to exactly 256, so the divide cannot lose the top of the range.
    /// Ward notes the green channel alone gives the same alignment results;
    /// these weights cost nothing over reading one channel.
    ///
    /// Panics unless `rgb.len() == width * height * 3`.
    pub fn from_rgb(rgb: &[u8], width: usize, height: usize) -> Self {
        assert_eq!(
            rgb.len(),
            width * height * 3,
            "{width}x{height} needs {} samples, got {}",
            width * height * 3,
            rgb.len()
        );

        let data = rgb
            .chunks_exact(3)
            .map(|pixel| {
                let (red, green, blue) = (pixel[0] as u32, pixel[1] as u32, pixel[2] as u32);
                ((54 * red + 183 * green + 19 * blue) / 256) as u8
            })
            .collect();

        Self {
            data,
            width,
            height,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub fn sample(&self, x: usize, y: usize) -> u8 {
        self.data[y * self.width + x]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_vec_carries_samples_and_dimensions() {
        let gray = Gray::from_vec(vec![1, 2, 3, 4, 5, 6], 3, 2);

        assert_eq!(gray.width(), 3);
        assert_eq!(gray.height(), 2);
        assert_eq!(gray.as_slice(), &[1, 2, 3, 4, 5, 6]);
    }

    #[test]
    #[should_panic(expected = "3x2 needs 6 samples, got 5")]
    fn from_vec_rejects_a_length_that_contradicts_the_dimensions() {
        Gray::from_vec(vec![1, 2, 3, 4, 5], 3, 2);
    }

    #[test]
    fn sample_reads_row_major() {
        let gray = Gray::from_vec(vec![1, 2, 3, 4, 5, 6], 3, 2);

        assert_eq!(gray.sample(0, 0), 1);
        assert_eq!(gray.sample(2, 0), 3);
        assert_eq!(gray.sample(0, 1), 4);
        assert_eq!(gray.sample(2, 1), 6);
    }

    /// The paper's weights sum to exactly 256, so white survives the divide
    /// intact. Anything that drifts off 255 here has lost a channel or rounded.
    #[test]
    fn from_rgb_maps_white_to_full_scale_and_black_to_zero() {
        let gray = Gray::from_rgb(&[255, 255, 255, 0, 0, 0], 2, 1);

        assert_eq!(gray.as_slice(), &[255, 0]);
    }

    /// grey = (54*red + 183*green + 19*blue) / 256. Green carries most of the
    /// weight, which is why Ward says the green channel alone would do.
    #[test]
    fn from_rgb_weights_the_channels_as_the_paper_does() {
        let gray = Gray::from_rgb(&[255, 0, 0, 0, 255, 0, 0, 0, 255], 3, 1);

        assert_eq!(gray.as_slice(), &[53, 182, 18]);
    }

    #[test]
    #[should_panic(expected = "2x1 needs 6 samples, got 5")]
    fn from_rgb_rejects_a_length_that_contradicts_the_dimensions() {
        Gray::from_rgb(&[1, 2, 3, 4, 5], 2, 1);
    }
}

use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct VectorOutput {
    pub id: u64,
    pub score: Option<f32>,
    pub payload: HashMap<String, String>,
}

/// A trait for utility methods on a list of VectorOutput.
pub trait VectorOutputListUtils {
    /// Sort the VectorOutputList in-place by score in descending order.
    ///
    /// This method uses the `sort_by` method of Vec to sort the elements in-place based on the result of a comparison function.
    /// The `partial_cmp` method is used to compare two Option<f32> values in a way that treats None as less than Some.
    fn sort_by_score(&mut self);

    /// Filter out results with scores below a given threshold.
    ///
    /// This method uses the `retain` method of Vec to keep only the elements specified by the predicate.
    /// The `map_or` method is used to return the provided value if the `Option` is `None`, or apply a function to the contained value if `Some`.
    /// In this case, it checks if the score is `Some` and if it's greater than the threshold.
    ///
    /// # Arguments
    ///
    /// * `score` - The threshold score. Results with scores below this value will be removed.
    fn limit_results(&mut self, score: f32);
}
pub type VectorOutputList = Vec<VectorOutput>;

impl VectorOutputListUtils for VectorOutputList {
    // A method to sort the outputs in descending order of score
    fn sort_by_score(&mut self) {
        // Sort the VectorOutputList in-place by score in descending order
        // The `sort_by` method sorts the elements in-place based on the result of a comparison function
        // The `partial_cmp` method compares two Option<f32> values in a way that treats None as less than Some
        self.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .expect("Ensure you sort a vectorlist which has scores set")
        });
    }
    // A method to filter out results with scores below a given threshold
    fn limit_results(&mut self, score: f32) {
        // Filter out results with scores below the threshold
        // The `retain` method keeps only the elements specified by the predicate
        // The `map_or` method returns the provided value if the `Option` is `None`, or applies a function to the contained value if `Some`
        // In this case, it checks if the score is `Some` and if it's greater than the threshold
        self.retain(|output| output.score.map_or(false, |s| s > score));
    }
}

#[derive(Debug, Clone)]
pub struct VectorInput {
    pub id: u64,
    pub embedding: Vec<f32>,
    pub payload: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_by_score() {
        let mut outputs = vec![
            VectorOutput {
                id: 1,
                score: Some(0.3),
                payload: HashMap::new(),
            },
            VectorOutput {
                id: 2,
                score: Some(0.5),
                payload: HashMap::new(),
            },
            VectorOutput {
                id: 3,
                score: Some(0.1),
                payload: HashMap::new(),
            },
        ];

        outputs.sort_by_score();

        assert_eq!(outputs[0].id, 2);
        assert_eq!(outputs[1].id, 1);
        assert_eq!(outputs[2].id, 3);
    }

    #[test]
    fn test_limit_results() {
        let mut output_list = vec![
            VectorOutput {
                score: Some(0.5),
                ..VectorOutput::default()
            },
            VectorOutput {
                score: Some(0.8),
                ..VectorOutput::default()
            },
            VectorOutput {
                score: Some(0.3),
                ..VectorOutput::default()
            },
            VectorOutput {
                score: None,
                ..VectorOutput::default()
            },
        ];

        output_list.limit_results(0.4);

        assert_eq!(output_list.len(), 2);
        assert_eq!(output_list[0].score, Some(0.5));
        assert_eq!(output_list[1].score, Some(0.8));
    }
}

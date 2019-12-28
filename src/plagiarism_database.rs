use crate::string_compare::is_plagiarised;
use crate::text_utils::{clean_text, extract_clean_word_ngrams};
use crate::Metric;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;

type TextOwnerID = String;
/// (start index (inclusive), end index (exclusive))
type FragmentLocation = (usize, usize);

/// Report for plagiarism between two owners
#[derive(Serialize)]
pub struct PlagiarismResult {
    pub owner_id1: TextOwnerID,
    pub owner_id2: TextOwnerID,
    pub matching_fragments: Vec<(String, String)>,
    pub trusted_owner1: bool,  // Is the first owner a trusted source?
    pub equal_fragments: bool, // Can we ignore one element of the tuple?
}

/// A single user's "submission" or text string, broken into fragments
#[derive(Debug)]
struct TextEntry {
    owner: TextOwnerID,
    /// Cleaned text (word-by-word) for usage in printing
    clean_text_words: Vec<String>,
    /// Unique string fragments in the text
    fragments: HashSet<String>,
    /// Mapping between fragment strings and where in the text they are located
    fragment_locations: HashMap<String, Vec<FragmentLocation>>,
}

/// Stores the corpus of trusted and untrusted strings
#[derive(Debug)]
pub struct PlagiarismDatabase {
    // Constant value for ngram size
    n: usize,
    // Constant value for metric cutoff value
    s: usize,
    // Metric to use
    metric: Metric,
    /// Map
    trusted_texts: Vec<TextEntry>,
    ///
    untrusted_texts: Vec<TextEntry>,
}

impl PlagiarismDatabase {
    /// Initializes the plagiarism sensitivity and similarity metric values
    ///     and the actual metric type to be used in computing plagiarism
    ///     scores
    pub fn new(n: usize, s: usize, metric: Metric) -> PlagiarismDatabase {
        PlagiarismDatabase {
            n,
            s,
            metric,
            trusted_texts: Vec::new(),
            untrusted_texts: Vec::new(),
        }
    }

    /// Adds a text string as potential plagiarism source material
    pub fn add_trusted_text(&mut self, owner_id: String, text: &str) {
        let clean_text_words = clean_text(text);
        let (fragments, fragment_locations) =
            PlagiarismDatabase::get_textfragments(&clean_text_words, self.n);
        self.trusted_texts.push(TextEntry {
            owner: owner_id,
            clean_text_words,
            fragments,
            fragment_locations,
        });
    }

    // Adds a text string as a potential plagiarized string
    pub fn add_untrusted_text(&mut self, owner_id: String, text: &str) {
        let clean_text_words = clean_text(text);
        let (fragments, fragment_locations) =
            PlagiarismDatabase::get_textfragments(&clean_text_words, self.n);
        self.untrusted_texts.push(TextEntry {
            owner: owner_id,
            clean_text_words,
            fragments,
            fragment_locations,
        });
    }

    /// Check for plagiarism by comparing metric against cutoff
    ///     for all textfragments currently in database
    pub fn check_untrusted_plagiarism(&self) -> Vec<PlagiarismResult> {
        let mut results: Vec<PlagiarismResult> = Vec::new();
        for i in 0..self.untrusted_texts.len() {
            for j in (i + 1)..self.untrusted_texts.len() {
                let source = &self.untrusted_texts[i];
                let against = &self.untrusted_texts[j];

                let matching_fragments = match self.metric {
                    Metric::Equal => self.check_plagiarism_equal(source, against),
                    _ => self.check_plagiarism_other(source, self.metric, against),
                };
                if matching_fragments.is_empty() {
                    continue;
                }
                let result = PlagiarismResult {
                    owner_id1: source.owner.clone(),
                    owner_id2: against.owner.clone(),
                    matching_fragments: matching_fragments,
                    trusted_owner1: false,
                    equal_fragments: self.metric == Metric::Equal,
                };
                results.push(result);
            }
        }
        results
    }

    /// Check for plagiarism by comparing metric against cutoff
    ///     for textfragments in database against trusted fragments
    pub fn check_trusted_plagiarism(&self) -> Vec<PlagiarismResult> {
        let mut results: Vec<PlagiarismResult> = Vec::new();
        println!("\n\nChecking against trusted sources...\n");
        for i in 0..self.trusted_texts.len() {
            for j in 0..self.untrusted_texts.len() {
                let source = &self.trusted_texts[i];
                let against = &self.untrusted_texts[j];
                let matching_fragments = match self.metric {
                    Metric::Equal => self.check_plagiarism_equal(source, against),
                    _ => self.check_plagiarism_other(source, self.metric, against),
                };
                if matching_fragments.is_empty() {
                    continue;
                }
                let result = PlagiarismResult {
                    owner_id1: source.owner.clone(),
                    owner_id2: against.owner.clone(),
                    matching_fragments: matching_fragments,
                    trusted_owner1: true,
                    equal_fragments: self.metric == Metric::Equal,
                };
                results.push(result);
            }
        }
        results
    }

    /// Splits a text string into separate ngram TextFragments
    ///     Also creates the map of fragments -> locations at the same time before
    ///     vector location information is lost
    fn get_textfragments(
        words: &Vec<String>,
        n: usize,
    ) -> (HashSet<String>, HashMap<String, Vec<FragmentLocation>>) {
        let ngrams = extract_clean_word_ngrams(words, n);
        let mut fragment_locations: HashMap<String, Vec<FragmentLocation>> = HashMap::new();
        let mut start_location = 0;
        // Insert all ngrams into hashmap of ngram locations
        // Handle both the case with no key (new vec) and existing key (push)
        for ngram in &ngrams {
            if fragment_locations.contains_key(ngram) {
                fragment_locations
                    .get_mut(ngram)
                    .unwrap()
                    .push((start_location, start_location + n));
            } else {
                let mut loc_vec: Vec<FragmentLocation> = Vec::new();
                loc_vec.push((start_location, start_location + n));
                fragment_locations.insert(ngram.to_string(), loc_vec);
            }
            start_location += 1;
        }
        (HashSet::from_iter(ngrams), fragment_locations)
    }

    /// Checks plagiarism by equality of fragments, uses fast set intersection
    /// Returns a tuple of all matches (second tuple element is identical to first)
    fn check_plagiarism_equal(
        &self,
        source: &TextEntry,
        against: &TextEntry,
    ) -> Vec<(String, String)> {
        let intersect: Vec<&String> = source.fragments.intersection(&against.fragments).collect();
        if !intersect.is_empty() {
            // Plagiarism!
            intersect
                .iter()
                .map(|val| (val.to_string(), val.to_string()))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Checks plagiarism by non-equal metric (string-by-string)
    /// Returns a tuple of all matches (second tuple element is identical to first)
    fn check_plagiarism_other(
        &self,
        source: &TextEntry,
        metric: Metric,
        against: &TextEntry,
    ) -> Vec<(String, String)> {
        let mut results: Vec<(String, String)> = Vec::new();
        for source_frag in source.fragments.iter() {
            for against_frag in against.fragments.iter() {
                if is_plagiarised(source_frag, against_frag, metric, self.s) {
                    results.push((source_frag.to_string(), against_frag.to_string()));
                }
            }
        }
        results
    }
}

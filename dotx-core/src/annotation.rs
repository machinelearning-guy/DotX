use crate::types::*;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub seqname: String,
    pub source: String,
    pub feature_type: String,
    pub start: GenomicPos,
    pub end: GenomicPos,
    pub score: Option<f64>,
    pub strand: Option<Strand>,
    pub phase: Option<u8>,
    pub attributes: HashMap<String, String>,
}

impl Annotation {
    pub fn length(&self) -> GenomicPos {
        self.end.saturating_sub(self.start)
    }

    pub fn overlaps(&self, start: GenomicPos, end: GenomicPos) -> bool {
        self.start < end && start < self.end
    }

    pub fn get_attribute(&self, key: &str) -> Option<&String> {
        self.attributes.get(key)
    }

    pub fn get_gene_name(&self) -> Option<&String> {
        self.get_attribute("gene")
            .or_else(|| self.get_attribute("gene_name"))
            .or_else(|| self.get_attribute("Name"))
    }

    pub fn get_id(&self) -> Option<&String> {
        self.get_attribute("ID")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationTrack {
    pub name: String,
    pub source_file: PathBuf,
    pub annotations: Vec<Annotation>,
    pub feature_types: Vec<String>,
    pub contig_index: HashMap<String, Vec<usize>>, // contig -> annotation indices
}

impl AnnotationTrack {
    pub fn new(name: String, source_file: PathBuf) -> Self {
        Self {
            name,
            source_file,
            annotations: Vec::new(),
            feature_types: Vec::new(),
            contig_index: HashMap::new(),
        }
    }

    pub fn add_annotation(&mut self, annotation: Annotation) {
        let index = self.annotations.len();
        
        // Update contig index
        self.contig_index
            .entry(annotation.seqname.clone())
            .or_insert_with(Vec::new)
            .push(index);
        
        // Update feature types
        if !self.feature_types.contains(&annotation.feature_type) {
            self.feature_types.push(annotation.feature_type.clone());
        }
        
        self.annotations.push(annotation);
    }

    pub fn get_annotations_in_region(&self, contig: &str, start: GenomicPos, end: GenomicPos) -> Vec<&Annotation> {
        if let Some(indices) = self.contig_index.get(contig) {
            indices
                .iter()
                .map(|&i| &self.annotations[i])
                .filter(|annotation| annotation.overlaps(start, end))
                .collect()
        } else {
            Vec::new()
        }
    }

    pub fn get_feature_types(&self) -> &[String] {
        &self.feature_types
    }
}

pub struct Gff3Reader {
    reader: BufReader<File>,
}

impl Gff3Reader {
    pub fn new(path: &PathBuf) -> Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Ok(Self { reader })
    }

    pub fn read_track(self, track_name: String, source_file: PathBuf) -> Result<AnnotationTrack> {
        let mut track = AnnotationTrack::new(track_name, source_file);
        
        for line in self.reader.lines() {
            let line = line?;
            let line = line.trim();
            
            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            
            let annotation = Self::parse_gff3_line_static(line)?;
            track.add_annotation(annotation);
        }
        
        Ok(track)
    }

    fn parse_gff3_line_static(line: &str) -> Result<Annotation> {
        let fields: Vec<&str> = line.split('\t').collect();
        
        if fields.len() != 9 {
            return Err(anyhow::anyhow!("GFF3 line must have 9 fields: {}", line));
        }

        let seqname = fields[0].to_string();
        let source = fields[1].to_string();
        let feature_type = fields[2].to_string();
        let start: GenomicPos = fields[3].parse()?;
        let end: GenomicPos = fields[4].parse()?;
        
        let score = if fields[5] == "." {
            None
        } else {
            Some(fields[5].parse()?)
        };

        let strand = match fields[6] {
            "+" => Some(Strand::Forward),
            "-" => Some(Strand::Reverse),
            "." => None,
            _ => return Err(anyhow::anyhow!("Invalid strand: {}", fields[6])),
        };

        let phase = if fields[7] == "." {
            None
        } else {
            Some(fields[7].parse()?)
        };

        let attributes = Self::parse_gff3_attributes_static(fields[8])?;

        Ok(Annotation {
            seqname,
            source,
            feature_type,
            start,
            end,
            score,
            strand,
            phase,
            attributes,
        })
    }

    fn parse_gff3_attributes_static(attr_string: &str) -> Result<HashMap<String, String>> {
        let mut attributes = HashMap::new();
        
        for pair in attr_string.split(';') {
            let pair = pair.trim();
            if pair.is_empty() {
                continue;
            }
            
            if let Some((key, value)) = pair.split_once('=') {
                let key = key.trim().to_string();
                let value = value.trim().to_string();
                
                // For now, just use the value directly (URL decoding would need urlencoding crate)
                let decoded_value = value;
                
                attributes.insert(key, decoded_value);
            }
        }
        
        Ok(attributes)
    }
}

pub struct GenBankReader {
    content: String,
}

impl GenBankReader {
    pub fn new(path: &PathBuf) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self { content })
    }

    pub fn read_track(&self, track_name: String, source_file: PathBuf) -> Result<AnnotationTrack> {
        let mut track = AnnotationTrack::new(track_name, source_file);
        
        // Simple GenBank parser - in reality this would be much more complex
        let mut in_features = false;
        let mut current_seqname = "unknown".to_string();
        
        for line in self.content.lines() {
            let line = line.trim();
            
            if line.starts_with("LOCUS") {
                if let Some(name) = line.split_whitespace().nth(1) {
                    current_seqname = name.to_string();
                }
            } else if line.starts_with("FEATURES") {
                in_features = true;
            } else if in_features && line.starts_with("ORIGIN") {
                break;
            } else if in_features && !line.is_empty() && !line.starts_with(' ') {
                // Feature line
                if let Ok(annotation) = self.parse_genbank_feature(line, &current_seqname) {
                    track.add_annotation(annotation);
                }
            }
        }
        
        Ok(track)
    }

    fn parse_genbank_feature(&self, line: &str, seqname: &str) -> Result<Annotation> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            return Err(anyhow::anyhow!("Invalid GenBank feature line: {}", line));
        }

        let feature_type = parts[0].to_string();
        let location = parts[1];

        // Parse location - this is very simplified
        let (start, end, strand) = self.parse_genbank_location(location)?;

        let mut attributes = HashMap::new();
        attributes.insert("source".to_string(), "genbank".to_string());

        Ok(Annotation {
            seqname: seqname.to_string(),
            source: "GenBank".to_string(),
            feature_type,
            start,
            end,
            score: None,
            strand,
            phase: None,
            attributes,
        })
    }

    fn parse_genbank_location(&self, location: &str) -> Result<(GenomicPos, GenomicPos, Option<Strand>)> {
        // Very simplified location parsing
        // Real GenBank locations can be much more complex
        
        let (location, strand) = if location.starts_with("complement(") && location.ends_with(')') {
            (&location[11..location.len()-1], Some(Strand::Reverse))
        } else {
            (location, Some(Strand::Forward))
        };

        if let Some((start_str, end_str)) = location.split_once("..") {
            let start: GenomicPos = start_str.parse()?;
            let end: GenomicPos = end_str.parse()?;
            Ok((start, end, strand))
        } else {
            // Single position
            let pos: GenomicPos = location.parse()?;
            Ok((pos, pos + 1, strand))
        }
    }
}
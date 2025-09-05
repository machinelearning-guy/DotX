//! Error handling for DOTx CLI

use thiserror::Error;
use std::path::PathBuf;

/// Main error type for DOTx CLI operations
#[derive(Error, Debug)]
pub enum CliError {
    #[error("Configuration error: {message}")]
    Config { message: String },
    
    #[error("Input/Output error: {message}")]
    Io { message: String },
    
    #[error("File not found: {path}")]
    FileNotFound { path: PathBuf },
    
    #[error("Invalid format: {message}")]
    InvalidFormat { message: String },
    
    #[error("Parsing error in {file}: {message}")]
    Parse { file: String, message: String },
    
    #[error("Seeding engine error: {engine} - {message}")]
    Seeding { engine: String, message: String },
    
    #[error("Alignment engine error: {engine} - {message}")]
    Alignment { engine: String, message: String },
    
    #[error("Database error: {message}")]
    Database { message: String },
    
    #[error("Rendering error: {message}")]
    Rendering { message: String },
    
    #[error("External tool error: {tool} - {message}")]
    ExternalTool { tool: String, message: String },
    
    #[error("Validation error: {message}")]
    Validation { message: String },
    
    #[error("Resource error: {message}")]
    Resource { message: String },
}

impl CliError {
    pub fn config<S: Into<String>>(message: S) -> Self {
        Self::Config { message: message.into() }
    }
    
    pub fn io<S: Into<String>>(message: S) -> Self {
        Self::Io { message: message.into() }
    }
    
    pub fn file_not_found(path: PathBuf) -> Self {
        Self::FileNotFound { path }
    }
    
    pub fn invalid_format<S: Into<String>>(message: S) -> Self {
        Self::InvalidFormat { message: message.into() }
    }
    
    pub fn parse<S: Into<String>>(file: S, message: S) -> Self {
        Self::Parse {
            file: file.into(),
            message: message.into(),
        }
    }
    
    pub fn seeding<S: Into<String>>(engine: S, message: S) -> Self {
        Self::Seeding {
            engine: engine.into(),
            message: message.into(),
        }
    }
    
    pub fn alignment<S: Into<String>>(engine: S, message: S) -> Self {
        Self::Alignment {
            engine: engine.into(),
            message: message.into(),
        }
    }
    
    pub fn database<S: Into<String>>(message: S) -> Self {
        Self::Database { message: message.into() }
    }
    
    pub fn rendering<S: Into<String>>(message: S) -> Self {
        Self::Rendering { message: message.into() }
    }
    
    pub fn external_tool<S: Into<String>>(tool: S, message: S) -> Self {
        Self::ExternalTool {
            tool: tool.into(),
            message: message.into(),
        }
    }
    
    pub fn validation<S: Into<String>>(message: S) -> Self {
        Self::Validation { message: message.into() }
    }
    
    pub fn resource<S: Into<String>>(message: S) -> Self {
        Self::Resource { message: message.into() }
    }
}

impl From<std::io::Error> for CliError {
    fn from(err: std::io::Error) -> Self {
        Self::io(err.to_string())
    }
}

impl From<toml::de::Error> for CliError {
    fn from(err: toml::de::Error) -> Self {
        Self::config(format!("TOML parsing error: {}", err))
    }
}

impl From<toml::ser::Error> for CliError {
    fn from(err: toml::ser::Error) -> Self {
        Self::config(format!("TOML serialization error: {}", err))
    }
}

/// Result type for CLI operations
pub type CliResult<T> = Result<T, CliError>;

/// Provide helpful error messages and suggestions
pub fn format_error_with_suggestions(error: &CliError) -> String {
    let mut message = error.to_string();
    
    // Add helpful suggestions based on error type
    match error {
        CliError::FileNotFound { path } => {
            message.push_str(&format!(
                "\n\nSuggestions:\n\
                 • Check that the file path is correct: {}\n\
                 • Ensure you have read permissions for the file\n\
                 • For FASTA files, ensure they are not compressed (or use .gz extension if they are)",
                path.display()
            ));
        }
        
        CliError::InvalidFormat { .. } => {
            message.push_str(
                "\n\nSuggestions:\n\
                 • Check the file format specification\n\
                 • Use --format to explicitly specify the input format\n\
                 • Ensure the file is not corrupted or truncated"
            );
        }
        
        CliError::ExternalTool { tool, .. } => {
            match tool.as_str() {
                "minimap2" => {
                    message.push_str(
                        "\n\nSuggestions:\n\
                         • Install minimap2: https://github.com/lh3/minimap2\n\
                         • Ensure minimap2 is in your PATH\n\
                         • Try using a different seeding engine with --engine"
                    );
                }
                _ => {
                    message.push_str(&format!(
                        "\n\nSuggestions:\n\
                         • Install {}\n\
                         • Ensure {} is in your PATH\n\
                         • Check that you have the required permissions",
                        tool, tool
                    ));
                }
            }
        }
        
        CliError::Config { .. } => {
            message.push_str(
                "\n\nSuggestions:\n\
                 • Check your dotx.toml configuration file\n\
                 • Use 'dotx config --example' to generate a sample configuration\n\
                 • Verify that all configuration values are valid"
            );
        }
        
        CliError::Database { .. } => {
            message.push_str(
                "\n\nSuggestions:\n\
                 • Check that the .dotxdb file is not corrupted\n\
                 • Try reimporting your data\n\
                 • Ensure you have write permissions for database updates"
            );
        }
        
        CliError::Resource { .. } => {
            message.push_str(
                "\n\nSuggestions:\n\
                 • Reduce the number of threads with --threads\n\
                 • Use a smaller batch size for processing\n\
                 • Free up system memory\n\
                 • Consider processing data in smaller chunks"
            );
        }
        
        _ => {}
    }
    
    message
}

/// Print error with helpful suggestions and exit
pub fn print_error_and_exit(error: &CliError) -> ! {
    eprintln!("Error: {}", format_error_with_suggestions(error));
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_error_creation() {
        let err = CliError::config("test message");
        assert!(matches!(err, CliError::Config { .. }));
        assert_eq!(err.to_string(), "Configuration error: test message");
    }
    
    #[test]
    fn test_error_suggestions() {
        let err = CliError::file_not_found(PathBuf::from("test.fa"));
        let formatted = format_error_with_suggestions(&err);
        assert!(formatted.contains("Suggestions:"));
        assert!(formatted.contains("Check that the file path is correct"));
    }
    
    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let cli_err: CliError = io_err.into();
        assert!(matches!(cli_err, CliError::Io { .. }));
    }
}
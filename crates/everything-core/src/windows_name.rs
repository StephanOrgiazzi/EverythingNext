use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsNameError {
    EmptyOrSurroundedByWhitespace,
    InvalidCharacter,
    Reserved,
    TooLong,
}

impl fmt::Display for WindowsNameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EmptyOrSurroundedByWhitespace => {
                "The name cannot be empty or start or end with a space."
            }
            Self::InvalidCharacter => {
                "This name contains a character that is not allowed on Windows."
            }
            Self::Reserved => "This name is reserved by Windows.",
            Self::TooLong => "The name cannot be longer than 255 UTF-16 code units.",
        })
    }
}

pub fn validate_windows_name(name: &str) -> Result<(), WindowsNameError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed != name {
        return Err(WindowsNameError::EmptyOrSurroundedByWhitespace);
    }
    if name.ends_with('.')
        || name.chars().any(|character| {
            character < '\u{20}'
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
        })
    {
        return Err(WindowsNameError::InvalidCharacter);
    }

    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end_matches(['.', ' '])
        .to_ascii_uppercase();
    let reserved = matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$" | "CLOCK$"
    ) || stem.strip_prefix("COM").is_some_and(|suffix| {
        matches!(
            suffix,
            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
        )
    }) || stem.strip_prefix("LPT").is_some_and(|suffix| {
        matches!(
            suffix,
            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
        )
    });
    if reserved {
        return Err(WindowsNameError::Reserved);
    }
    if name.encode_utf16().count() > 255 {
        return Err(WindowsNameError::TooLong);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_windows_name, WindowsNameError};

    #[test]
    fn validates_windows_file_names() {
        for name in [
            "rapport final.pdf",
            "photo_été.png",
            "archive.tar.gz",
            "README",
        ] {
            assert_eq!(validate_windows_name(name), Ok(()), "{name}");
        }
    }

    #[test]
    fn rejects_each_invalid_name_category() {
        assert_eq!(
            validate_windows_name(" fichier.txt"),
            Err(WindowsNameError::EmptyOrSurroundedByWhitespace)
        );
        assert_eq!(
            validate_windows_name("bad\u{1f}.txt"),
            Err(WindowsNameError::InvalidCharacter)
        );
        assert_eq!(
            validate_windows_name("con.txt"),
            Err(WindowsNameError::Reserved)
        );
        assert_eq!(
            validate_windows_name(&"a".repeat(256)),
            Err(WindowsNameError::TooLong)
        );
    }
}

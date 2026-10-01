//! What went wrong, said so that someone can do something about it. Every failure a person can
//! run into — a share code that will not read, a picture that will not open, an export that could
//! not be written, an update that did not arrive — comes through here on its way to the notebook,
//! so each says what happened, whether anything was lost, and what to try next, rather than
//! passing on an error meant for a log.
use formiga_core::SeedCodeError;

/// Why a pasted share code was not accepted, and what to try.
pub fn share_code(error: &SeedCodeError) -> String {
    match error {
        SeedCodeError::Prefix => {
            "That does not look like a Formiga code. A companion's code begins with FORMIGA- \
             — copy it again from your friend's notebook, from Copy code on their companion's \
             page."
        }
        SeedCodeError::Format | SeedCodeError::Length => {
            "Part of the code seems to be missing. Codes are groups of four letters and numbers \
             separated by dashes; make sure the whole code was copied, with nothing cut off at \
             either end."
        }
        SeedCodeError::Character => {
            "The code has a character in it that no Formiga code uses. Codes use only capital \
             letters and the digits 2 to 7, so a 0, 1, 8 or 9 was probably typed by mistake — \
             copying and pasting is safer than typing."
        }
        SeedCodeError::Checksum => {
            "One or more characters in the code are wrong, so it does not check out. Copy it \
             again rather than retyping it."
        }
        SeedCodeError::Version | SeedCodeError::Generation | SeedCodeError::Design => {
            "This code was made by a newer Formiga than this one, so it cannot be read here yet. \
             Check for updates on the About page, then paste it again."
        }
    }
    .to_owned()
}

/// The first input/output error inside an error chain, if there is one.
fn io_kind(error: &anyhow::Error) -> Option<std::io::ErrorKind> {
    error
        .chain()
        .find_map(|cause| cause.downcast_ref::<std::io::Error>())
        .map(std::io::Error::kind)
}

/// Why something chosen to be written to disk — a card, a sticker, a portrait, a postcard or a
/// backup — was not written, and what to try. `what` names the thing: "the postcard".
pub fn export(what: &str, error: &anyhow::Error) -> String {
    use std::io::ErrorKind;
    let detail = format!("{error:#}");
    let lower = detail.to_lowercase();
    let advice = match io_kind(error) {
        Some(ErrorKind::PermissionDenied | ErrorKind::ReadOnlyFilesystem) => {
            "Formiga is not allowed to write there. Choose another folder, such as Pictures or \
             Documents."
        }
        Some(ErrorKind::StorageFull | ErrorKind::QuotaExceeded) => {
            "The disk is full. Free some space, or choose another drive."
        }
        Some(ErrorKind::NotFound) => {
            "The folder chosen is no longer there — a drive may have been disconnected. Choose \
             another one."
        }
        _ if lower.contains("no space") => {
            "The disk is full. Free some space, or choose another drive."
        }
        _ => "Nothing in your colony was changed. Try again, or choose another folder.",
    };
    format!("Could not save {what}. {advice} ({detail})")
}

/// Why a picture could not be used to find companions in, and what to try.
pub fn reference_image(error: &anyhow::Error) -> String {
    let detail = format!("{error:#}");
    let lower = detail.to_lowercase();
    let advice = if lower.contains("larger than 16 mb") {
        "That picture is larger than 16 MB. Choose a smaller one, or save a smaller copy of it \
         first."
    } else if lower.contains("too many pixels") || lower.contains("limit") {
        "That picture is too big to look at — up to 4,096 pixels on a side is fine. Save a \
         smaller copy of it and choose that."
    } else if lower.contains("png or jpeg") {
        "Only PNG and JPEG pictures can be used. Export or save the picture as one of those \
         first."
    } else if lower.contains("decode") || lower.contains("recognize") {
        "That file could not be read as a picture — it may be damaged, or not really the kind \
         of file its name says. Try opening it in another app and saving a fresh PNG copy."
    } else if matches!(
        io_kind(error),
        Some(std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::NotFound)
    ) {
        "Formiga could not open that file. It may have been moved, or be somewhere Formiga is \
         not allowed to read; try copying it to your Pictures folder first."
    } else {
        "Try another picture."
    };
    format!("Could not use that picture. {advice}")
}

/// Why a colony file — a backup chosen to restore, or the colony itself at launch — could not be
/// read, and what that means.
pub fn colony_file(error: &formiga_core::PersistenceError) -> String {
    use formiga_core::PersistenceError;
    match error {
        PersistenceError::UnsupportedVersion(version) => format!(
            "It was written by a newer Formiga (save version {version}) than this one can read. \
             Update Formiga from the About page, then open it again; the file is untouched."
        ),
        PersistenceError::Json(detail) => format!(
            "It is not a Formiga colony, or it has been damaged — it may have been cut short \
             while copying. Try another copy of it. ({detail})"
        ),
        PersistenceError::Io(io) if io.kind() == std::io::ErrorKind::PermissionDenied => {
            "Formiga is not allowed to read it. Copy it somewhere like Documents and choose that \
             copy."
                .to_owned()
        }
        PersistenceError::Io(io) if io.kind() == std::io::ErrorKind::InvalidData => format!(
            "It is larger than any colony file can be, so it is not a Formiga backup. ({io})"
        ),
        PersistenceError::Io(io) => format!("It could not be read. ({io})"),
    }
}

/// What an update failure means, and what to try: a headline and the advice under it.
pub fn update(message: &str) -> (&'static str, String) {
    let lower = message.to_lowercase();
    if lower.contains("sha-256")
        || lower.contains("checksum")
        || lower.contains("unexpected size")
        || lower.contains("exceeded the advertised")
    {
        (
            "The downloaded update did not check out",
            "It was thrown away without being opened, because it did not match what the release \
             says it should be. Nothing was installed. Try again in a little while; if it keeps \
             happening, download it from the project's releases page instead."
                .to_owned(),
        )
    } else if lower.contains("403") || lower.contains("rate limit") {
        (
            "GitHub is not answering just now",
            "GitHub limits how often it is asked. Try again in an hour or so.".to_owned(),
        )
    } else if lower.contains("dns")
        || lower.contains("resolve")
        || lower.contains("connect")
        || lower.contains("timed out")
        || lower.contains("network")
        || lower.contains("io:")
    {
        (
            "Could not reach GitHub",
            "Check that this computer is online, then try again. Your colony is not affected."
                .to_owned(),
        )
    } else if lower.contains("installer") || lower.contains("launch") || lower.contains("open") {
        (
            "The update could not be opened",
            format!(
                "The verified download is still there to try again. If it will not open, \
                 download it from the project's releases page instead. ({message})"
            ),
        )
    } else {
        (
            "The update did not go through",
            format!("Nothing was installed, and your colony is not affected. ({message})"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_share_code_failure_says_what_to_do() {
        for error in [
            SeedCodeError::Prefix,
            SeedCodeError::Format,
            SeedCodeError::Version,
            SeedCodeError::Generation,
            SeedCodeError::Character,
            SeedCodeError::Length,
            SeedCodeError::Checksum,
            SeedCodeError::Design,
        ] {
            let said = share_code(&error);
            assert!(said.len() > 60, "{error:?}: {said}");
            assert!(
                ["copy", "Copy", "update", "pasting", "copied"]
                    .iter()
                    .any(|verb| said.contains(verb)),
                "{error:?} gives nothing to try: {said}"
            );
        }
        assert!(share_code(&SeedCodeError::Character).contains("2 to 7"));
    }

    #[test]
    fn an_export_failure_names_the_cause_when_it_can() {
        let denied =
            anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
                .context("write postcard");
        let said = export("the postcard", &denied);
        assert!(said.starts_with("Could not save the postcard. Formiga is not allowed"));
        let full = anyhow::Error::from(std::io::Error::from(std::io::ErrorKind::StorageFull));
        assert!(export("the sticker", &full).contains("disk is full"));
        let other = anyhow::anyhow!("encoder hiccup");
        let said = export("the card", &other);
        assert!(said.contains("Nothing in your colony was changed"));
        assert!(
            said.contains("encoder hiccup"),
            "the detail is kept for reference"
        );
    }

    #[test]
    fn a_picture_failure_says_what_kind_of_picture_would_work() {
        let wrong = anyhow::anyhow!("choose a PNG or JPEG image");
        assert!(reference_image(&wrong).contains("Only PNG and JPEG"));
        let big = anyhow::anyhow!("reference image is larger than 16 MB");
        assert!(reference_image(&big).contains("smaller"));
        let broken = anyhow::anyhow!("invalid chunk").context("decode reference image");
        assert!(reference_image(&broken).contains("damaged"));
    }

    #[test]
    fn update_failures_are_told_apart() {
        assert_eq!(
            update("downloaded installer failed SHA-256 verification").0,
            "The downloaded update did not check out"
        );
        assert_eq!(
            update("http status: 403").0,
            "GitHub is not answering just now"
        );
        assert_eq!(
            update("io: failed to lookup address information (dns error)").0,
            "Could not reach GitHub"
        );
        assert_eq!(update("something new").0, "The update did not go through");
    }
}

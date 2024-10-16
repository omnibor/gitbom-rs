use crate::Error;
use crate::Result;
use hyperpolyglot::detect as detect_language;
use hyperpolyglot::Language as KnownLanguage;
use std::fmt::Debug;
use std::fs::read;
use std::fs::File;
use std::ops::Not;
use std::path::Path;

#[allow(unused)]
#[derive(Debug)]
pub(crate) enum TargetType {
    KnownBinaryType(BinaryType),
    KnownTextType(TextType),
    Unknown,
}

impl TargetType {
    pub(crate) fn infer(path: &Path, _file: &File) -> Result<Self> {
        match detect_binary_type(path)? {
            Detection::SupportedBinaryType(binary_type) => {
                return Ok(TargetType::KnownBinaryType(binary_type));
            }
            Detection::UnsupportedBinaryType(unsupported_binary_type) => {
                return Err(Error::UnsupportedBinaryType(unsupported_binary_type));
            }
            // Keep going! Assume text from here on out.
            Detection::Uncertain => {}
        }

        let detection = detect_language(path)
            .map_err(|err| Error::CantReadTarget {
                path: path.to_owned(),
                err,
            })?
            .ok_or_else(|| Error::CantDetectLanguage(path.to_owned()))?;

        // SAFETY: Per the hyperpolyglot docs, this is always valid when the
        // input to the `try_from` conversion came from a call to `detect`.
        let _language = KnownLanguage::try_from(detection.language()).unwrap();

        // This gets us a language name, but doesn't tell us the information
        // we really want, unfortunately. What we care about is whether it's
        // a known binary file, or a format which accepts comments, and if it
        // accepts comments, what kind of comments it accepts.
        //
        // I wish `hyperpolyglot` had a `KnownLanguages` type, marked
        // non-exhaustive, which was an enum generated from the list of
        // languages, which implemented `Display` and `FromStr`.
        //
        // Under the structure we'll likely end up with, hyperpolyglot will
        // handle producing known and trusted names of languages, where we
        // can feel pretty confident that the name of a detected language
        // matches what that language actually _is_. It won't be perfect, and
        // we'll eventually want to offer some means of this inference being
        // overridden, but it will be useful.
        //
        // So the part we'll need to do is to produce the `KnownLanguages`
        // enum ourselves.
        //
        // Oh, one thing I've just realized is that Linguist, and by extension
        // hyperpolyglot, does _not_ do detection of ELF files or any other
        // binary format. So we'll need something else for that.
        //
        // The flow will likely end up being:
        //
        // 1. Detect if it's a text file or binary file.
        // 2. If it's a text file, use hyperpolyglot to infer a language. If
        //    it's a binary, use `infer` to infer the binary format.
        // 3. Use our own mapping of language or binary format name to known
        //    ones, to produce our final inferred `TargetType`.

        todo!("inferring target file type is not yet implemented")
    }
}

#[allow(unused)]
#[derive(Debug)]
pub(crate) enum BinaryType {
    ElfFile,
}

#[allow(unused)]
#[derive(Debug)]
pub(crate) enum TextType {
    PrefixComments { prefix: String },
    WrappedComments { prefix: String, suffix: String },
}

enum Detection {
    SupportedBinaryType(BinaryType),
    UnsupportedBinaryType(String),
    Uncertain,
}

fn detect_binary_type(path: &Path) -> Result<Detection> {
    // Read the contents of the file.
    let buf = read(path).map_err(|err| Error::CantReadTarget {
        path: path.to_owned(),
        err,
    })?;

    // If it's ELF, we're done.
    if infer::app::is_elf(&buf) {
        return Ok(Detection::SupportedBinaryType(BinaryType::ElfFile));
    }

    // If it's another known format, report that.
    if let Some(kind) = infer::get(&buf) {
        if matches!(
            kind.mime_type(),
            "text/html" | "text/xml" | "text/x-shellscript"
        )
        .not()
        {
            return Ok(Detection::UnsupportedBinaryType(
                kind.extension().to_owned(),
            ));
        }
    }

    Ok(Detection::Uncertain)
}

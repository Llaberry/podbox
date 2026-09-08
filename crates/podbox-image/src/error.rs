//! One typed error for the crate, converted at the boundary.
//!
//! `docs/conventions/code.md`: typed errors, never a bare string, and never an
//! error swallowed silently. Every variant here carries enough to name the
//! thing that failed without the caller re-deriving it.

use std::fmt;

/// docker's exit code for "the runtime could not run the command". The CLI
/// maps everything here onto it except [`Error::Usage`].
pub const EXIT_RUNTIME_ERROR: i32 = 125;
/// `pathshim`'s contract, adopted at
/// `references/compforge__pathshim/tree/README.md:65`: invalid input exits 2.
pub const EXIT_USAGE: i32 = 2;

#[derive(Debug)]
pub enum Error {
    /// The caller wrote something this crate cannot act on.
    Usage(String),
    /// A reference that does not parse, with what was wrong.
    Reference(String),
    /// ⛔ `TODO/image.md` T-0201: a registry offering only `http://` is a named
    /// refusal, not a downgrade. tcp/80 egress hangs on the studied runtime, so
    /// a fallback that exists to improve reliability makes the failure untimed.
    PlainHttpRefused(String),
    /// Transport, DNS, TLS, or a status this client will not act on.
    Http { what: String, detail: String },
    /// ⛔ `TODO/image.md` T-0202: computed and declared digests differ. Both are
    /// named, because "digest mismatch" alone cannot be acted on.
    DigestMismatch {
        what: String,
        want: String,
        got: String,
    },
    /// ⛔ `TODO/image.md` T-0203: the destination, the free amount, the required
    /// amount and the unit, so the message can be acted on without a second
    /// command. The mistake this exists to avoid is in the corpus at
    /// `references/mhx__dwarfs/tree/src/utility/filesystem_extractor.cpp:544-552`.
    NoSpace(String),
    /// A malformed or unsupported OCI document, naming the media type.
    Oci(String),
    /// The store, its layout, or a lock.
    Store(String),
    /// Anything the filesystem refused, with the path.
    Io {
        path: String,
        source: std::io::Error,
    },
    /// A reference that named nothing this store holds.
    NoSuchImage(String),
}

impl Error {
    pub fn io(path: impl Into<String>, source: std::io::Error) -> Error {
        Error::Io {
            path: path.into(),
            source,
        }
    }

    /// The process exit code this error produces. ⚠ Only a caller mistake is
    /// 2; everything else is docker's 125, so a shell that branches on 2 is
    /// branching on its own input and not on the network.
    pub fn exit_code(&self) -> i32 {
        match self {
            // ⚠ `PlainHttpRefused` is here because it names something the
            // CALLER wrote: `podbox pull http://...`. A registry whose token
            // realm is `http://` is not caller input and is an `Http` error
            // with the same explanation, so the two do not share a code.
            Error::Usage(_) | Error::Reference(_) | Error::PlainHttpRefused(_) => EXIT_USAGE,
            _ => EXIT_RUNTIME_ERROR,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Usage(m) => write!(f, "{m}"),
            Error::Reference(m) => write!(f, "invalid reference: {m}"),
            Error::PlainHttpRefused(m) => write!(
                f,
                "{m}: podbox speaks HTTPS only and will not downgrade. \
                 tcp/80 egress is broken on the runtime podbox targets, so a \
                 plain-HTTP fallback hangs instead of failing (TODO/image.md T-0201)"
            ),
            Error::Http { what, detail } => write!(f, "{what}: {detail}"),
            Error::DigestMismatch { what, want, got } => write!(
                f,
                "{what}: digest mismatch, expected {want} and computed {got}. \
                 The bytes were discarded"
            ),
            Error::NoSpace(m) => write!(f, "{m}"),
            Error::Oci(m) => write!(f, "{m}"),
            Error::Store(m) => write!(f, "store: {m}"),
            Error::Io { path, source } => write!(f, "{path}: {source}"),
            Error::NoSuchImage(r) => write!(f, "no such image: {r}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

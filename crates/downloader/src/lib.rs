pub mod composite;
pub mod filename;
pub mod http;
pub mod telegram;
pub mod traits;
pub mod ytdlp;
pub mod ytdlp_errors;

pub use composite::CompositeDownloader;
pub use filename::{filename_from_url_path, sanitize_filename};
pub use traits::FileDownloader;
pub use ytdlp::{build_args, bytes_to_ytdlp_max_filesize, run_ytdlp, YtDlpArgs, YtDlpRunError};
pub use ytdlp_errors::{
    classify_ytdlp_stderr, failure_kind_slug, friendly_ytdlp_message, user_message_for_kind,
    YtDlpFailureKind,
};

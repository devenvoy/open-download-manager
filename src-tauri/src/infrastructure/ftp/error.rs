use crate::application::error::AppError;

#[derive(Debug)]
pub enum FtpError {
    ConnectFailed(String),
    Protocol(String),
    AuthFailed(String),
    FileNotFound(String),
    DataConnectionFailed(String),
    TransferAborted(String),
    RestRejected(String),
    Io(std::io::Error),
}

impl std::fmt::Display for FtpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectFailed(msg) => write!(f, "FTP connection failed: {msg}"),
            Self::Protocol(msg) => write!(f, "FTP protocol error: {msg}"),
            Self::AuthFailed(msg) => write!(f, "FTP authentication failed: {msg}"),
            Self::FileNotFound(msg) => write!(f, "FTP file not found: {msg}"),
            Self::DataConnectionFailed(msg) => write!(f, "FTP data connection failed: {msg}"),
            Self::TransferAborted(msg) => write!(f, "FTP transfer aborted: {msg}"),
            Self::RestRejected(msg) => write!(f, "FTP resume offset rejected: {msg}"),
            Self::Io(err) => write!(f, "FTP I/O error: {err}"),
        }
    }
}

impl std::error::Error for FtpError {}

impl From<std::io::Error> for FtpError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl From<FtpError> for AppError {
    fn from(err: FtpError) -> Self {
        match err {
            FtpError::AuthFailed(msg) => AppError {
                code: "ftp_auth_failed",
                message: if msg.is_empty() {
                    "FTP authentication failed. Check your username and password.".to_owned()
                } else {
                    format!("FTP authentication failed: {msg}")
                },
            },
            FtpError::FileNotFound(msg) => AppError {
                code: "ftp_file_not_found",
                message: if msg.is_empty() {
                    "The requested FTP file was not found on the server.".to_owned()
                } else {
                    format!("The requested FTP file was not found: {msg}")
                },
            },
            FtpError::DataConnectionFailed(msg) => AppError {
                code: "ftp_data_failed",
                message: format!("Could not establish FTP data connection: {msg}"),
            },
            FtpError::TransferAborted(msg) => AppError {
                code: "ftp_transfer_aborted",
                message: format!("FTP transfer was aborted: {msg}"),
            },
            FtpError::ConnectFailed(msg) => AppError::network(format!("FTP connection failed: {msg}")),
            FtpError::Protocol(msg) => AppError::download(format!("FTP protocol error: {msg}")),
            FtpError::RestRejected(msg) => AppError::download(format!("FTP resume rejected: {msg}")),
            FtpError::Io(err) => AppError::download(format!("FTP transfer I/O error: {err}")),
        }
    }
}

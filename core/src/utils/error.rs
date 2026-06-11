use thiserror::Error;

#[derive(Error, Debug)]
#[allow(dead_code)]
pub enum AirDropdError {
    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Device discovery error: {0}")]
    DiscoveryError(String),

    #[error("Protocol error: {0}")]
    ProtocolError(String),

    #[error("Connection timeout")]
    ConnectionTimeout,

    #[error("Invalid network interface: {0}")]
    InvalidInterface(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl AirDropdError {
    pub fn is_temporary(&self) -> bool {
        matches!(self, 
            AirDropdError::ConnectionTimeout |
            AirDropdError::NetworkError(_)
        )
    }

    pub fn should_retry(&self) -> bool {
        self.is_temporary()
    }
}

#[allow(dead_code)]
pub type AirDropdResult<T> = Result<T, AirDropdError>;

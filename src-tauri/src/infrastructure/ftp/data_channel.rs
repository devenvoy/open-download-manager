// Created: 2026-11-08
// Reason: feat(transfer): add FTP transfers with passive mode, authentication, and resume (Closes #28)
// Last updated: 2026-11-08
// Reason: feat(transfer): add FTP transfers with passive mode, authentication, and resume (Closes #28)
// Description: FTP data channel handler supporting active and passive transfer modes.

use std::{net::SocketAddr, time::Duration};

use tokio::{
    net::{TcpListener, TcpStream},
    time::timeout,
};

use super::{connection::FtpConnection, error::FtpError};

pub enum FtpDataChannel {
    Passive(TcpStream),
    Active(TcpListener),
}

impl FtpDataChannel {
    pub async fn open_passive(
        conn: &mut FtpConnection,
        prefer_epsv: bool,
        connect_timeout: Duration,
    ) -> Result<TcpStream, FtpError> {
        let addr_res = if prefer_epsv {
            match conn.send_epsv().await {
                Ok(addr) => Ok(addr),
                Err(_) => conn.send_pasv().await,
            }
        } else {
            conn.send_pasv().await
        };

        let target_addr = addr_res?;
        match timeout(connect_timeout, TcpStream::connect(target_addr)).await {
            Ok(Ok(stream)) => Ok(stream),
            Ok(Err(err)) => Err(FtpError::DataConnectionFailed(format!(
                "Failed to connect to data port {target_addr}: {err}"
            ))),
            Err(_) => Err(FtpError::DataConnectionFailed(format!(
                "Timed out connecting to data port {target_addr}"
            ))),
        }
    }

    pub async fn setup_active(
        conn: &mut FtpConnection,
        active_ip: Option<std::net::IpAddr>,
    ) -> Result<TcpListener, FtpError> {
        let bind_ip = active_ip.unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
        let listener = TcpListener::bind(SocketAddr::new(bind_ip, 0))
            .await
            .map_err(FtpError::Io)?;
        let local_addr = listener.local_addr().map_err(FtpError::Io)?;

        // Send PORT command to server with our listener address
        conn.send_port(local_addr).await?;
        Ok(listener)
    }

    pub async fn accept_active(
        listener: TcpListener,
        accept_timeout: Duration,
    ) -> Result<TcpStream, FtpError> {
        match timeout(accept_timeout, listener.accept()).await {
            Ok(Ok((stream, _))) => Ok(stream),
            Ok(Err(err)) => Err(FtpError::DataConnectionFailed(format!(
                "Failed to accept active data connection: {err}"
            ))),
            Err(_) => Err(FtpError::DataConnectionFailed(
                "Timed out waiting for server to connect to active data listener".to_owned(),
            )),
        }
    }
}

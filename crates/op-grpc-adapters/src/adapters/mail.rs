//! Mail adapter — bridges IMAP/SMTP to gRPC.
//!
//! Connects to mail-3tched container via unix sockets:
//!   IMAP:  /run/mail-3tched/imap.sock
//!   SMTP:  /run/mail-3tched/submission.sock

use crate::proto::{
    mail_service_server::MailService, DeleteMessageRequest, DeleteMessageResponse,
    GetMessageRequest, GetMessageResponse, ListFoldersRequest, ListFoldersResponse,
    ListMessagesRequest, ListMessagesResponse, MailEvent, MailHeader, MoveMessageRequest,
    MoveMessageResponse, SearchMessagesRequest, SearchMessagesResponse, SendMessageRequest,
    SendMessageResponse, StreamInboxRequest,
};
use async_trait::async_trait;
use std::pin::Pin;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

const IMAP_SOCK: &str = "/run/mail-3tched/imap.sock";
const SUBMISSION_SOCK: &str = "/run/mail-3tched/submission.sock";

pub struct MailAdapter {
    imap_addr: String,
    smtp_addr: String,
}

impl MailAdapter {
    pub fn new() -> Self {
        Self {
            imap_addr: IMAP_SOCK.to_string(),
            smtp_addr: SUBMISSION_SOCK.to_string(),
        }
    }

    async fn imap_session(&self) -> Result<async_imap::Session<tokio::net::UnixStream>, Status> {
        let stream = tokio::net::UnixStream::connect(&self.imap_addr)
            .await
            .map_err(|e| Status::unavailable(format!("IMAP socket unavailable: {}", e)))?;

        let user = std::env::var("MAIL_USER").unwrap_or_default();
        let pass = std::env::var("MAIL_PASS").unwrap_or_default();

        let client = async_imap::Client::new(stream);
        client
            .login(&user, &pass)
            .await
            .map_err(|(e, _)| Status::unauthenticated(format!("IMAP login failed: {}", e)))
    }
}

impl Default for MailAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MailService for MailAdapter {
    async fn list_folders(
        &self,
        _req: Request<ListFoldersRequest>,
    ) -> Result<Response<ListFoldersResponse>, Status> {
        let mut session = self.imap_session().await?;
        let names = session
            .list(Some(""), Some("*"))
            .await
            .map_err(|e| Status::internal(format!("IMAP LIST failed: {}", e)))?;

        use futures::StreamExt;
        let folders: Vec<String> = names
            .filter_map(|n| async move { n.ok().map(|n| n.name().to_string()) })
            .collect()
            .await;

        session
            .logout()
            .await
            .map_err(|e| Status::internal(format!("IMAP logout failed: {}", e)))?;

        Ok(Response::new(ListFoldersResponse { folders }))
    }

    async fn list_messages(
        &self,
        req: Request<ListMessagesRequest>,
    ) -> Result<Response<ListMessagesResponse>, Status> {
        let r = req.into_inner();
        let mut session = self.imap_session().await?;

        session
            .select(&r.folder)
            .await
            .map_err(|e| Status::not_found(format!("Folder not found: {}", e)))?;

        let limit = if r.limit == 0 { 50 } else { r.limit };
        let seq = format!("{}:{}", r.offset + 1, r.offset + limit);

        let messages = session
            .fetch(&seq, "(UID FLAGS ENVELOPE)")
            .await
            .map_err(|e| Status::internal(format!("IMAP FETCH failed: {}", e)))?;

        use futures::TryStreamExt;
        let raw_msgs: Vec<_> = messages
            .try_collect()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        // stream dropped here — session borrow released

        let headers: Vec<MailHeader> = raw_msgs
            .iter()
            .filter_map(|m| {
                let env = m.envelope()?;
                let subject = env
                    .subject
                    .as_ref()
                    .map(|s| String::from_utf8_lossy(s).to_string())
                    .unwrap_or_default();
                let from = env
                    .from
                    .as_ref()
                    .and_then(|f| f.first())
                    .map(|a| {
                        format!(
                            "{}@{}",
                            a.mailbox
                                .as_ref()
                                .map(|m| String::from_utf8_lossy(m).to_string())
                                .unwrap_or_default(),
                            a.host
                                .as_ref()
                                .map(|h| String::from_utf8_lossy(h).to_string())
                                .unwrap_or_default(),
                        )
                    })
                    .unwrap_or_default();
                Some(MailHeader {
                    uid: m.uid.unwrap_or(0) as u64,
                    subject,
                    from,
                    to: String::new(),
                    date: String::new(),
                    seen: m
                        .flags()
                        .any(|f| matches!(f, async_imap::types::Flag::Seen)),
                })
            })
            .collect();

        session.logout().await.ok();
        Ok(Response::new(ListMessagesResponse { messages: headers }))
    }

    async fn get_message(
        &self,
        req: Request<GetMessageRequest>,
    ) -> Result<Response<GetMessageResponse>, Status> {
        let r = req.into_inner();
        let mut session = self.imap_session().await?;

        session
            .select(&r.folder)
            .await
            .map_err(|e| Status::not_found(format!("Folder not found: {}", e)))?;

        let messages = session
            .uid_fetch(r.uid.to_string(), "(UID FLAGS ENVELOPE BODY[])")
            .await
            .map_err(|e| Status::internal(format!("IMAP UID FETCH failed: {}", e)))?;

        use futures::TryStreamExt;
        let raw: Vec<_> = messages
            .try_collect()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        // stream dropped — session borrow released

        let msg = raw
            .into_iter()
            .next()
            .ok_or_else(|| Status::not_found("Message not found"))?;

        let body = msg
            .body()
            .map(|b| String::from_utf8_lossy(b).to_string())
            .unwrap_or_default();

        let header = MailHeader {
            uid: msg.uid.unwrap_or(0) as u64,
            subject: msg
                .envelope()
                .and_then(|e| e.subject.as_ref())
                .map(|s| String::from_utf8_lossy(s).to_string())
                .unwrap_or_default(),
            from: String::new(),
            to: String::new(),
            date: String::new(),
            seen: msg
                .flags()
                .any(|f| matches!(f, async_imap::types::Flag::Seen)),
        };

        drop(msg);
        session.logout().await.ok();
        Ok(Response::new(GetMessageResponse {
            header: Some(header),
            body_text: body,
            body_html: String::new(),
            attachments: vec![],
        }))
    }

    async fn search_messages(
        &self,
        req: Request<SearchMessagesRequest>,
    ) -> Result<Response<SearchMessagesResponse>, Status> {
        let r = req.into_inner();
        let mut session = self.imap_session().await?;
        session.select(&r.folder).await.ok();

        let criteria = format!("TEXT \"{}\"", r.query);
        let uids = session
            .uid_search(criteria)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let uid_list: Vec<String> = uids.iter().map(|u| u.to_string()).collect();
        let seq = uid_list.join(",");

        let messages = session
            .uid_fetch(&seq, "(UID FLAGS ENVELOPE)")
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        use futures::TryStreamExt;
        let raw_msgs: Vec<_> = messages
            .try_collect()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let results: Vec<MailHeader> = raw_msgs
            .iter()
            .map(|m| MailHeader {
                uid: m.uid.unwrap_or(0) as u64,
                subject: m
                    .envelope()
                    .and_then(|e| e.subject.as_ref())
                    .map(|s| String::from_utf8_lossy(s).to_string())
                    .unwrap_or_default(),
                from: String::new(),
                to: String::new(),
                date: String::new(),
                seen: m
                    .flags()
                    .any(|f| matches!(f, async_imap::types::Flag::Seen)),
            })
            .collect();

        session.logout().await.ok();
        Ok(Response::new(SearchMessagesResponse { messages: results }))
    }

    async fn move_message(
        &self,
        req: Request<MoveMessageRequest>,
    ) -> Result<Response<MoveMessageResponse>, Status> {
        let r = req.into_inner();
        let mut session = self.imap_session().await?;
        session.select(&r.from_folder).await.ok();
        session
            .uid_mv(r.uid.to_string(), &r.to_folder)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        session.logout().await.ok();
        Ok(Response::new(MoveMessageResponse { success: true }))
    }

    async fn delete_message(
        &self,
        req: Request<DeleteMessageRequest>,
    ) -> Result<Response<DeleteMessageResponse>, Status> {
        let r = req.into_inner();
        let mut session = self.imap_session().await?;
        session.select(&r.folder).await.ok();
        let _ = session
            .uid_store(r.uid.to_string(), "+FLAGS (\\Deleted)")
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        session.expunge().await.ok();
        session.logout().await.ok();
        Ok(Response::new(DeleteMessageResponse { success: true }))
    }

    type StreamInboxStream = Pin<Box<dyn Stream<Item = Result<MailEvent, Status>> + Send>>;

    async fn stream_inbox(
        &self,
        _req: Request<StreamInboxRequest>,
    ) -> Result<Response<Self::StreamInboxStream>, Status> {
        // TODO: implement IMAP IDLE once async-imap IDLE API is stabilised
        Err(Status::unimplemented(
            "IMAP IDLE streaming not yet implemented",
        ))
    }

    async fn send_message(
        &self,
        req: Request<SendMessageRequest>,
    ) -> Result<Response<SendMessageResponse>, Status> {
        let r = req.into_inner();
        // Use the submission socket via a raw SMTP conversation
        let stream = tokio::net::UnixStream::connect(&self.smtp_addr)
            .await
            .map_err(|e| Status::unavailable(format!("SMTP socket unavailable: {}", e)))?;

        // Build a minimal RFC 5321 SMTP transaction over the unix socket
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        let (reader, mut writer) = tokio::io::split(stream);
        let mut lines = BufReader::new(reader).lines();

        // Read banner (expect "220 ...")
        let banner = lines
            .next_line()
            .await
            .map_err(|e| Status::internal(format!("SMTP banner read failed: {}", e)))?
            .ok_or_else(|| Status::internal("SMTP connection closed before banner"))?;
        if !banner.starts_with("220") {
            return Err(Status::internal(format!(
                "unexpected SMTP banner: {}",
                banner
            )));
        }

        let from = std::env::var("MAIL_FROM").unwrap_or_else(|_| "noreply@3tched.com".to_string());
        let msg_id = uuid::Uuid::new_v4().to_string();

        for (cmd, expected_code) in [
            ("EHLO localhost\r\n".to_string(), "250"),
            (format!("MAIL FROM:<{}>\r\n", from), "250"),
            (format!("RCPT TO:<{}>\r\n", r.to), "250"),
            ("DATA\r\n".to_string(), "354"),
        ] {
            writer
                .write_all(cmd.as_bytes())
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            let response = lines
                .next_line()
                .await
                .map_err(|e| Status::internal(format!("SMTP response read failed: {}", e)))?
                .ok_or_else(|| Status::internal("SMTP connection closed unexpectedly"))?;
            if !response.starts_with(expected_code) {
                return Err(Status::internal(format!(
                    "SMTP command {:?} rejected: {}",
                    cmd.trim_end(),
                    response
                )));
            }
        }

        let body = format!(
            "Message-ID: <{}>\r\nFrom: {}\r\nTo: {}\r\nSubject: {}\r\nContent-Type: text/plain\r\n\r\n{}\r\n.\r\n",
            msg_id, from, r.to, r.subject, r.body_text
        );
        writer
            .write_all(body.as_bytes())
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let data_response = lines
            .next_line()
            .await
            .map_err(|e| Status::internal(format!("SMTP DATA response read failed: {}", e)))?
            .ok_or_else(|| Status::internal("SMTP connection closed before DATA confirmation"))?;
        if !data_response.starts_with("250") {
            return Err(Status::internal(format!(
                "message rejected by SMTP server: {}",
                data_response
            )));
        }

        writer.write_all(b"QUIT\r\n").await.ok();

        Ok(Response::new(SendMessageResponse {
            success: true,
            message_id: msg_id,
        }))
    }
}

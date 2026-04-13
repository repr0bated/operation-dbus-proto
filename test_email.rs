use lettre::{Message, transport::smtp::authentication::Credentials, transport::smtp::client::Tls, AsyncSmtpTransport, AsyncTransport, Tokio1Executor};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let from_name = "Operation DBUS Registration";
    let from_email = "noreply@3tched.com";
    let to_email = "test@example.com";
    
    let mailbox_str = format!("{} <{}>", from_name, from_email);
    println!("Parsing mailbox: {}", mailbox_str);
    let mailbox: lettre::message::Mailbox = mailbox_str.parse()?;
    println!("Mailbox parsed successfully");
    
    Ok(())
}

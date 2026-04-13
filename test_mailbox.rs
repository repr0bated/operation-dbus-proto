use lettre::message::Mailbox;

fn main() {
    let from_name = "Operation DBUS Registration";
    let from_email = "noreply@3tched.com";
    let mailbox_str = format!("{} <{}>", from_name, from_email);
    
    match mailbox_str.parse::<Mailbox>() {
        Ok(_) => println!("Success: {}", mailbox_str),
        Err(e) => println!("Error parsing mailbox: {}", e),
    }

    let quoted_name = format!("\"{}\"", from_name);
    let quoted_mailbox_str = format!("{} <{}>", quoted_name, from_email);
    match quoted_mailbox_str.parse::<Mailbox>() {
        Ok(_) => println!("Success: {}", quoted_mailbox_str),
        Err(e) => println!("Error parsing quoted mailbox: {}", e),
    }
}

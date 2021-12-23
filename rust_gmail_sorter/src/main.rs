extern crate google_gmail1 as gmail1;
extern crate hyper;
extern crate hyper_rustls;
use gmail1::api::{Message, MessagePartHeader};
use gmail1::{oauth2, Gmail};
use gmail1::{Error, Result};
use itertools::Itertools;
use std::default::Default;
use std::fs;
use std::ops::Deref;
use pancurses::Input::Character;

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
enum Status {
    Inbox,
    FollowUp,
    ReadThrough,
    Archive,
}

#[derive(Debug)]
struct Email {
    status: Status,
    subject: String,
    from_name: String,
    from_domain: String,
    body: String,
    date: i64,
    id: String,
}

impl ToString for Status {
    fn to_string(&self) -> String {
        match self {
            Status::Inbox => "Inbox".to_owned(),
            Status::FollowUp => "FollowUp".to_owned(),
            Status::ReadThrough => "ReadThrough".to_owned(),
            Status::Archive => "Archive".to_owned(),
        }
    }
}

fn get_header(name: &str, headers: &[MessagePartHeader]) -> Option<String> {
    for h in headers {
        if h.name.is_none() {
            continue;
        }
        if name.eq_ignore_ascii_case(&h.name.as_deref().unwrap()) {
            return h.value.clone();
        }
    }
    None
}

impl Email {
    fn new(message: &gmail1::api::Message) -> Option<Self> {
        let headers = &message.payload.as_ref()?.headers.as_deref()?;
        let subject = get_header("subject", headers).unwrap_or_default();
        let from_raw = get_header("from", headers)?;
        let from_vec: Vec<_> = from_raw.split(",").collect_vec();
        let from = if from_vec.is_empty() || from_vec.len() > 1 {
            get_header("sender", headers).unwrap_or_default()
        } else {
            from_vec.first().unwrap().deref().to_owned()
        };
        let from_parts = from.split("@").collect_vec();
        let from_name = from_parts.get(0).unwrap_or(&"").deref().to_owned();
        let from_domain = from_parts.get(1).unwrap_or(&"").deref().to_owned();
        let date: i64 = message.internal_date.as_ref()?.parse().ok()?;
        Some(Email {
            status: Status::Inbox,
            subject,
            body: message.snippet.as_deref()?.to_owned(),
            from_name,
            from_domain,
            date,
            id: message.id.as_deref()?.to_owned(),
        })
    }
}

async fn fetch_inbox_ids(gmail: &Gmail) -> Option<Vec<String>> {
    let mut ids: Vec<String> = vec![];
    let mut page_token: Option<String> = None;
    loop {
        let query = gmail.users().messages_list("me").add_label_ids("INBOX");
        let query = if page_token.is_none() {
            query
        } else {
            query.page_token(&page_token.unwrap())
        };
        let response = query.doit().await;
        if response.is_err(){
            break;
        }
        let response = response.unwrap();
        page_token = response.1.next_page_token.clone();
        if page_token.is_none(){ break;}
        let current_ids = response.1.messages;
        if current_ids.is_none() {break;}
        let current_ids = current_ids.unwrap();
        for m in current_ids{
           if m.id.is_none() { continue;}
            ids.push(m.id.unwrap());
        }
    }

    Some(ids)


}

fn update_status(current: &mut Email, status: &Option<Status>) {
    if status.is_some() {
        current.status = status.clone().unwrap();
    }
}

async fn fetch_inbox<F:Fn(usize)>(gmail: &Gmail, f:F) -> Vec<Email> {
    let mut emails: Vec<Email> = vec![];
    let ids =  fetch_inbox_ids(gmail).await.unwrap_or_default();
    for id in &ids{
        f(emails.len());
        let result = gmail.users().messages_get("me",id).format("full").doit().await;
        if(result.is_err()) {continue;}
        let email = Email::new(&result.unwrap().1);
        if email.is_none() {continue;}
        emails.push(email.unwrap());
    }
    emails
}

#[tokio::main]
async fn main() {
    // Get an ApplicationSecret instance by some means. It contains the `client_id` and
    // `client_secret`, among other things.

    let secret = oauth2::read_application_secret("./client_secret.json")
        .await
        .expect("client_secret.json");

    // Create an authenticator that uses an InstalledFlow to authenticate. The
    // authentication tokens are persisted to a file named tokencache.json. The
    // authenticator takes care of caching tokens to disk and refreshing tokens once
    // they've expired.
    let mut auth = oauth2::InstalledFlowAuthenticator::builder(
        secret,
        oauth2::InstalledFlowReturnMethod::HTTPRedirect,
    )
    .build()
    .await
    .unwrap();

    let mut hub = Gmail::new(
        hyper::Client::builder().build(hyper_rustls::HttpsConnector::with_native_roots()),
        auth,
    );

   /* dotenv::dotenv().unwrap();

    let password = dotenv::var("IMAP_PASSWORD").ok().unwrap_or(String::new());
    let user = dotenv::var("IMAP_USER").ok().unwrap_or(String::new());
    let domain = dotenv::var("IMAP_DOMAIN").ok().unwrap_or(String::new());
    let follow_up = dotenv::var("IMAP_FOLLOWUP").ok().unwrap();
    let read_through = dotenv::var("IMAP_READTHROUGH").ok().unwrap();
    let archive = dotenv::var("IMAP_ARCHIVE").ok().unwrap();

    */

    let mut window = pancurses::initscr();
    let mut emails = fetch_inbox(&hub,|c| {
        window.clear();
        window.addstr(format!("Read {} emails", c));
        window.refresh();
    })
        .await;
    emails.sort_by(|a, b| {
        (&a.from_domain, &a.from_name, &a.date)
            .cmp(&(&b.from_domain, &b.from_name, &b.date))
            .reverse()
    });
    let mut i = 0usize;
    if emails.is_empty() {
        println!("No emails");
        return;
    }
    let mut needs_refresh = true;
    let mut command = String::new();
    let mut change_status: Option<Status> = None;
    loop {
        if (i >= emails.len()) {
            i = emails.len() - 1;
        }
        if needs_refresh {
            let text = format!(
                "Status: {} of {} {}\n\nDate:{} \n\nFrom:{}@{}\n\nSubject:{}\n\n{}",
                i + 1,
                emails.len(),
                emails[i].status.to_string(),
                emails[i].date,
                emails[i].from_name,
                emails[i].from_domain,
                emails[i].subject,
                emails[i].body
            );
            window.clear();
            window.addstr(text);
            let mut cy = window.get_cur_y();
            let y = window.get_max_y();
            while cy < y {
                window.addstr("\n");
                cy += 1;
            }
            window.mv(y, 0);
            window.refresh();
            window.clrtoeol();
            window.addstr(&format!("Command: {}", command));
            window.refresh();
            needs_refresh = false;
        }
        match window.getch().unwrap() {
            Character('q') => return,
            Character('a') => change_status = Some(Status::Archive),
            Character('f') => change_status = Some(Status::FollowUp),
            Character('r') => change_status = Some(Status::ReadThrough),
            Character('i') => change_status = Some(Status::Inbox),

            Character('0') => {
                i = 0;
                needs_refresh = true;
            }
            Character('j') => {
                update_status(&mut emails[i], &mut change_status);
                if i < emails.len() - 1 {
                    i += 1;
                }
                change_status = None;
                needs_refresh = true;
            }
            Character('k') => {
                update_status(&mut emails[i], &mut change_status);
                if i > 0 {
                    i -= 1;
                }
                change_status = None;
                needs_refresh = true;
            }
            Character('p') => {
                update_status(&mut emails[i], &mut change_status);
                let mut j = i;
                while j < emails.len() - 1
                    && emails[j].from_name == emails[i].from_name
                    && emails[j].from_domain == emails[i].from_domain
                {
                    update_status(&mut emails[j], &mut change_status);
                    j += 1;
                }
                i = j;
                change_status = None;
                needs_refresh = true;
            }
            Character('d') => {
                update_status(&mut emails[i], &mut change_status);
                let mut j = i;
                while j < emails.len() - 1 && emails[j].from_domain == emails[i].from_domain {
                    update_status(&mut emails[j], &mut change_status);
                    j += 1;
                }
                i = j;
                change_status = None;
                needs_refresh = true;
            },
            Character('w') =>{
              /*  move_emails(&emails,&domain,&user,&password,&archive,&follow_up,&read_through,
                            |s,count,total|{
                                window.clear();
                                window.addstr(format!("Moved {} of {} to {}",count, total,s));
                                window.refresh();
                            }).unwrap();

               */
                let mut v:Vec<_> =  emails.into_iter().filter(|e|e.status == Status::Inbox).collect();
                emails = v;
                if emails.is_empty() {
                    window.clear();
                    window.addstr(format!("No emails"));
                    window.refresh();
                    return;
                }
                i = 0;
            },
            pancurses::Input::KeyEnter => needs_refresh = true,

            _ => (),
        }
    }


}

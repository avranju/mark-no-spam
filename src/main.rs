use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::Result;
use google_gmail1::{api::BatchModifyMessagesRequest, Gmail};
use structopt::StructOpt;

const USER_ID: &'static str = "me";

#[derive(StructOpt, Debug)]
#[structopt(name = "no-spam")]
struct Opt {
    #[structopt(short, long)]
    client_config: PathBuf,

    #[structopt(short, long)]
    secret_cache: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let opt = Opt::from_args();

    let secret = yup_oauth2::read_application_secret(opt.client_config).await?;

    let auth = yup_oauth2::InstalledFlowAuthenticator::builder(
        secret,
        yup_oauth2::InstalledFlowReturnMethod::HTTPRedirect,
    )
    .persist_tokens_to_disk(opt.secret_cache)
    .build()
    .await?;

    let hub = Gmail::new(
        hyper::Client::builder().build(hyper_rustls::HttpsConnector::with_native_roots()),
        auth,
    );

    let mut page_token: Option<String> = None;
    let mut count: usize = 0;
    loop {
        let (messages, next_page_token) = list_messages(
            &hub,
            "in:spam",
            page_token.as_ref().map(AsRef::as_ref),
            1000,
        )
        .await?;

        count += messages.len();
        mark_not_spam(&hub, messages).await?;
        print!("Marked {} messages as not spam\r", count);
        io::stdout().flush()?;

        page_token = next_page_token;
        if page_token.is_none() {
            break;
        }
    }

    println!("\nAll done.");
    Ok(())
}

async fn mark_not_spam(hub: &Gmail, messages: Vec<String>) -> Result<()> {
    let req = BatchModifyMessagesRequest {
        add_label_ids: None,
        ids: Some(messages),
        remove_label_ids: Some(vec!["SPAM".to_string()]),
    };
    let _ = hub
        .users()
        .messages_batch_modify(req, USER_ID)
        .doit()
        .await?;

    Ok(())
}

async fn list_messages(
    hub: &Gmail,
    query: &str,
    next_page_token: Option<&str>,
    max_results: u32,
) -> Result<(Vec<String>, Option<String>)> {
    let req = hub
        .users()
        .messages_list(USER_ID)
        .q(query)
        .max_results(max_results);
    let req = if let Some(token) = next_page_token {
        req.page_token(token)
    } else {
        req
    };

    let (_, res) = req.doit().await?;

    if let Some(messages) = res.messages {
        Ok((
            messages.into_iter().filter_map(|m| m.id).collect(),
            res.next_page_token,
        ))
    } else {
        Ok((vec![], None))
    }
}

use glib::clone;
use gtk::{glib, Application, ApplicationWindow};
use gtk::{prelude::*, Button};
use serde::Deserialize;
use std::sync::OnceLock;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

#[derive(Deserialize, Debug)]
pub struct Channel {
    #[serde(rename = "GuideNumber")]
    pub guide_number: String,
    #[serde(rename = "GuideName")]
    pub guide_name: String,
    #[serde(rename = "VideoCodec")]
    pub video_codec: String,
    #[serde(rename = "AudioCodec")]
    pub audio_codec: String,
    #[serde(rename = "URL")]
    pub url: String,
    #[serde(rename = "HD")]
    pub hd: Option<i32>,
    #[serde(rename = "Favorite")]
    pub favorite: Option<i32>,
}

#[derive(Default)]
pub struct HdHomerunClient;
impl HdHomerunClient {
    const URI: &'static str = "http://10.0.0.4/lineup.json";
    pub async fn get_lineup(&mut self) -> Result<Vec<Channel>, reqwest::Error> {
        let url = reqwest::Url::parse(Self::URI).unwrap();
        let body = reqwest::get(url).await?.json::<Vec<Channel>>().await;
        match body {
            Ok(data) => Ok(data),
            Err(e) => panic!("{}", e),
        }
    }
}

fn runtime() -> &'static Runtime {
    static RUNTIME: OnceLock<Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| Runtime::new().expect("Setting up tokio runtime needs to succeed."))
}

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("org.gtk_rs.hdhomerun.channel_list")
        .build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &Application) {
    let (sender, receiver) = async_channel::bounded::<Result<Vec<Channel>, reqwest::Error>>(1);
    runtime().spawn(clone!(@strong sender => async move {
        let mut client = HdHomerunClient::default();
        let lineup= client.get_lineup().await;
        sender.send(lineup).await.expect("The channel needs to be open.");
    }));

    let list_box = gtk::ListBox::builder().build();
    let scrolled_window = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .min_content_width(360)
        .child(&list_box)
        .build();
    let child: Arc<std::sync::Mutex<Option<std::process::Child>>> = Arc::new(Mutex::new(None));
    glib::spawn_future_local(async move {
        while let Ok(response) = receiver.recv().await {
            match response {
                Ok(response) => {
                    for channel in response {
                        let button = gtk::Button::with_label(&channel.guide_name);
                        let child_clone = Arc::clone(&child);
                        button.connect_clicked(move |button| {
                            if let Some(ref mut old_c) = *child_clone.lock().unwrap() {
                                old_c.kill().expect("could not kill child");
                            }
                            use std::process::Command;
                            let handle = Command::new("mpv").arg(channel.url.clone()).spawn().ok();
                            *child_clone.lock().unwrap() = handle;
                        });
                        list_box.append(&button);
                    }
                }
                Err(_) => {
                    println!("bad request");
                }
            }
        }
    });

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Tokio integration")
        .default_width(600)
        .default_height(300)
        .child(&scrolled_window)
        .build();
    window.present();
}

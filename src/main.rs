#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use] extern crate rocket;

use std::time::Duration;

use chrono::{Datelike, NaiveTime, Utc};
use clokwerk::Interval::Weekday;
use clokwerk::Scheduler;
use rocket_contrib::json::Json;

use crate::config::*;
use crate::last_day::{get_last_workday, is_last_workday};
use slack::client::*;
use slack::events;
use slack::events::{SlackRequest, SlackEvents};
use rocket::State;

mod last_day;
mod config;

fn main() {
    let config = Configuration::read()
        .expect("couldn't read configuration file");
    let client = SlackClient::new()
        .expect("couldn't initiate slack client");

    // Run scheduler
    let mut scheduler = Scheduler::with_tz(chrono::Utc);
    scheduler.every(Weekday)
        .at_time(NaiveTime::from_hms(9, 0, 0))
        .run(move || {
            let now = Utc::now();
            match is_last_workday(&now) {
                Ok(true) => {
                    let context = now.date().month().to_string();
                    let message = config.get_message(&context);
                    match client.get_channel_id_by_name("allmant") {
                        Some(channel_id) => {
                            if let Err(error) = client.post_message(&channel_id, &message) {
                                println!("couldn't post message: {}", error)
                            }
                        }
                        None => println!("no channel with name 'allmant' found!")
                    }
                }
                Ok(false) => {
                    println!("not last work day")
                }
                Err(error) => {
                    println!("{}", error);
                }
            }
        });
    let _schedule_handle = scheduler.watch_thread(Duration::from_secs(60));

    // Setup slack_events handler
    let mut slack_events = events::SlackEvents::new();
    slack_events.set_mention_event_handler(handle_mention_event);

    // Start web server
    rocket::ignite()
        .mount("/", routes![slack_request])
        .manage(slack_events)
        .launch();
}

#[post("/slack-request", format = "application/json", data = "<request>")]
fn slack_request(state: State<SlackEvents>, request: Json<SlackRequest>) -> String {
    state.handle_request(request.0)
}

fn handle_mention_event(client: &SlackClient, event: events::AppMentionEvent) -> String {
    let config = Configuration::read()
        .expect("couldn't read configuration when mentioned");

    let splits: Vec<&str> = event.text.split(" ").collect();
    let message: String = if splits.len() > 1 {
        match splits[1] {
            "tid" => {
                let now = Utc::now();
                match get_last_workday(&now) {
                    Ok(last_workday) => {
                        if last_workday == now.naive_utc().date() {
                            format!("Okej, jag har kikat i kalendern och det är först *{}* som du behöver tidrapportera!\n\n... vänta\n... beräknar\n... det är ju idag!", last_workday)
                        } else {
                            format!("Okej, jag har kikat i kalendern och det är först *{}* som du behöver tidrapportera!", last_workday)
                        }
                    }
                    Err(error) => {
                        println!("{}", error);
                        String::from("Herregud någonting gick skitfel! Jag kanske behöver uppdatera min firmware :joel:. Kan någon snälla kolla loggen i Azure?")
                    }
                }
            }
            "pricing" => {
                String::from("För den nätta kostnaden av 114,805 kr per månad eller 15,8 öre per timme kan du hosta din egen joel-bot! :joel:")
            }
            "skribenter" => {
                config.get_authors()
            }
            command => {
                format!("Är du skön eller? Tror du att _jag_ vet något om *{}*? :joel:", command)
            }
        }
    } else {
        config.get_introduction()
    };
    client.post_message(&event.channel, &message)
        .unwrap_or_else(|error| println!("{}", error));

    String::new()
}

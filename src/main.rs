use std::env;
use slack_api::chat::{PostMessageRequest, PostMessageResponse};
use rusoto_ce::{CostExplorerClient, CostExplorer, GetCostAndUsageRequest, DateInterval, MetricValue, ResultByTime};
use rusoto_core::{Client, HttpClient, Region};
use reqwest::header::DATE;
use chrono::{Date, Utc, Duration};
use std::ops::Sub;
use std::str::FromStr;


const DAILY: &str = "DAILY";
const UNBLENDED_COST: &str = "UnblendedCost";

enum CostGranularityType {
    Monthly,
    Daily,
    Hourly,
}

impl CostGranularityType {
    fn name(self) -> String {
        match self {
            CostGranularityType::Monthly => "MONTHLY".to_owned(),
            CostGranularityType::Daily => "DAILY".to_owned(),
            CostGranularityType::Hourly => "HOURLY".to_owned(),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = env::var("SLACK_API_TOKEN")
        .map_err(|_| "SLACK_API_TOKEN env var must be set")?;
    let channel_id = env::var("SLACK_CHANNEL_ID")
        .map_err(|_| "SLACK_CHANNEL_ID env var must be set")?;

    let aws_cost_client = AwsCostClient::default();
    let cost = aws_cost_client
        .get_cost(CostGranularityType::Monthly)
        .await?;

    let message = format!("Usage cost: {:?} {}", cost.amount, cost.unit);
    let client = SlackClient::new(
        token.as_str(),
        channel_id.as_str(),
    )?;
    let response = client
        .send_message(message.as_str())
        .await?;
    println!("Message sent successfully {:?}", response);
    Ok(())
}

#[derive(Debug)]
struct Cost {
    amount: f32,
    unit: String,
}

fn get_date_interval_from_end_date(end_date: Date<Utc>) -> DateInterval {
    let before_one_day = end_date.sub(Duration::days(1));
    DateInterval {
        start: before_one_day.format("%Y-%m-%d").to_string(),
        end: end_date.format("%Y-%m-%d").to_string(),
    }
}

struct AwsCostClient {
    client: CostExplorerClient,
}

impl AwsCostClient {
    fn default() -> AwsCostClient {
        AwsCostClient {
            client: CostExplorerClient::new(Region::UsEast1)
        }
    }
    async fn get_cost(&self, cost_granularity_type: CostGranularityType) -> Result<Cost, String> {
        let response = self.client
            .get_cost_and_usage(
                GetCostAndUsageRequest {
                    granularity: Some(cost_granularity_type.name()),
                    time_period: get_date_interval_from_end_date(Utc::today()),
                    metrics: Some(vec!(UNBLENDED_COST.to_owned())),
                    ..Default::default()
                })
            .await
            .map_err(|_| "Request error")?;

        let results_by_times = response
            .results_by_time
            .unwrap_or(Vec::new());
        let first_result = results_by_times
            .first()
            .ok_or("Nothing first result".to_owned())?;
        let total_cost = first_result
            .total
            .as_ref()
            .ok_or("Error".to_owned())?;
        let metric_value = &total_cost[UNBLENDED_COST];
        let amount = metric_value
            .amount
            .as_ref()
            .unwrap_or(&"0".to_owned())
            .to_string();
        let amount = f32::from_str(amount.as_str())
            .map_err(|_| "Parse error Float32".to_owned())?;
        let unit = metric_value
            .unit
            .as_ref()
            .unwrap_or(&"".to_owned())
            .to_string();

        Ok(Cost {
            amount,
            unit,
        })
    }
}

struct SlackClient<'a> {
    client: reqwest::Client,
    token: &'a str,
    channel_id: &'a str,
}

impl<'a> SlackClient<'a> {
    fn new(token: &'a str, channel_id: &'a str) -> Result<Self, &'a str> {
        let client = slack_api::default_client().map_err(|_| "Could not get default_client")?;
        Ok(SlackClient {
            client,
            token,
            channel_id,
        })
    }

    async fn send_message(self, message: &str) -> Result<PostMessageResponse, &str> {
        slack_api::chat::post_message(&self.client, self.token, &PostMessageRequest {
            channel: self.channel_id,
            text: message,
            ..Default::default()
        }).await.map_err(|error| {
            println!("{:?}", error);
            "Could not send massage"
        })
    }
}
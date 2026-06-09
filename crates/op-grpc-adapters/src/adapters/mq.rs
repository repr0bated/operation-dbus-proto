//! MQTT adapter — bridges Mosquitto to gRPC streaming.
//! Connects via unix socket: /run/netmaker/mq.sock (MQTT :1883)

use crate::proto::{
    mq_service_server::MqService, ListTopicsRequest, ListTopicsResponse, MqMessage,
    PublishRequest, PublishResponse, SubscribeRequest,
};
use async_trait::async_trait;
use rumqttc::{AsyncClient, EventLoop, MqttOptions, QoS};
use std::pin::Pin;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

const MQTT_HOST: &str = "127.0.0.1";
const MQTT_PORT: u16 = 1883;

pub struct MqAdapter;

impl MqAdapter {
    pub fn new() -> Self {
        Self
    }

    fn mqtt_client(client_id: &str) -> (AsyncClient, EventLoop) {
        let mut opts = MqttOptions::new(client_id, MQTT_HOST, MQTT_PORT);
        opts.set_keep_alive(std::time::Duration::from_secs(30));
        if let (Ok(user), Ok(pass)) = (
            std::env::var("MQTT_USER"),
            std::env::var("MQTT_PASS"),
        ) {
            opts.set_credentials(user, pass);
        }
        AsyncClient::new(opts, 128)
    }
}

impl Default for MqAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MqService for MqAdapter {
    async fn publish(
        &self,
        req: Request<PublishRequest>,
    ) -> Result<Response<PublishResponse>, Status> {
        let r = req.into_inner();
        let qos = match r.qos {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            _ => QoS::ExactlyOnce,
        };

        let (client, mut eventloop) = Self::mqtt_client("op-grpc-adapter-pub");
        client
            .publish(&r.topic, qos, r.retain, r.payload)
            .await
            .map_err(|e| Status::internal(format!("MQTT publish failed: {}", e)))?;

        // Drive the event loop until the publish is acknowledged
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                match eventloop.poll().await {
                    Ok(rumqttc::Event::Outgoing(rumqttc::Outgoing::Publish(_))) => break,
                    Ok(_) => continue,
                    Err(e) => return Err(Status::internal(format!("MQTT event loop error: {}", e))),
                }
            }
            Ok(())
        })
        .await
        .map_err(|_| Status::deadline_exceeded("MQTT publish timed out"))??;

        Ok(Response::new(PublishResponse { success: true }))
    }

    type SubscribeStream = Pin<Box<dyn Stream<Item = Result<MqMessage, Status>> + Send>>;

    async fn subscribe(
        &self,
        req: Request<SubscribeRequest>,
    ) -> Result<Response<Self::SubscribeStream>, Status> {
        let r = req.into_inner();
        let qos = match r.qos {
            0 => QoS::AtMostOnce,
            1 => QoS::AtLeastOnce,
            _ => QoS::ExactlyOnce,
        };

        let (client, mut eventloop) = Self::mqtt_client(&format!(
            "op-grpc-adapter-sub-{}",
            uuid::Uuid::new_v4()
        ));

        for topic in &r.topics {
            client
                .subscribe(topic, qos)
                .await
                .map_err(|e| Status::internal(format!("MQTT subscribe failed: {}", e)))?;
        }

        let stream = async_stream::stream! {
            loop {
                match eventloop.poll().await {
                    Ok(rumqttc::Event::Incoming(rumqttc::Packet::Publish(p))) => {
                        yield Ok(MqMessage {
                            topic: p.topic,
                            payload: p.payload.to_vec(),
                            timestamp: std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs())
                                .unwrap_or(0),
                        });
                    }
                    Ok(_) => continue,
                    Err(e) => {
                        yield Err(Status::internal(format!("MQTT connection error: {}", e)));
                        break;
                    }
                }
            }
        };

        Ok(Response::new(Box::pin(stream)))
    }

    async fn list_topics(
        &self,
        _req: Request<ListTopicsRequest>,
    ) -> Result<Response<ListTopicsResponse>, Status> {
        // Subscribe to $SYS/# for broker stats and extract topic names
        let (client, mut eventloop) = Self::mqtt_client("op-grpc-adapter-topics");
        client
            .subscribe("$SYS/#", QoS::AtMostOnce)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let mut topics = Vec::new();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(2);

        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(
                std::time::Duration::from_millis(500),
                eventloop.poll(),
            )
            .await
            {
                Ok(Ok(rumqttc::Event::Incoming(rumqttc::Packet::Publish(p)))) => {
                    topics.push(p.topic);
                }
                _ => break,
            }
        }

        client.disconnect().await.ok();
        Ok(Response::new(ListTopicsResponse { topics }))
    }
}

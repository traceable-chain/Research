use rumqtt::{MqttClient, MqttOptions, Notification, QoS};
use std::{thread, time::Duration};

const MQTT_CLIENT_ID: &str = "test-pubsub1";
const MQTT_ADDRESS: &str = "localhost";
const MQTT_PORT: u16 = 1883;
const MQTT_TOPIC: &str = "bedroom/temperature";

fn connect_to_mqtt() -> Result<(MqttClient, rumqtt::Receiver<Notification>), rumqtt::ConnectError> {
    let mqtt_options = MqttOptions::new(MQTT_CLIENT_ID, MQTT_ADDRESS, MQTT_PORT);
    MqttClient::start(mqtt_options)
}

fn main() {
    match connect_to_mqtt() {
        Ok((mut mqtt_client, notifications)) => {
            mqtt_client.subscribe(MQTT_TOPIC, QoS::AtLeastOnce).unwrap();
            let sleep_time = Duration::from_secs(1);

            thread::spawn(move || {
                for i in 0..100 {
                    let payload = format!("publish {}", i);

                    thread::sleep(sleep_time);

                    mqtt_client
                        .publish(MQTT_TOPIC, QoS::AtLeastOnce, false, payload)
                        .unwrap();
                }
            });

            for notification in notifications {
                println!("{:?}", notification)
            }
        }
        Err(e) => println!("error: {:?}", e),
    }
}

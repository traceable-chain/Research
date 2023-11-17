use rumqtt::{MqttClient, MqttOptions, QoS};
use tokio;

#[tokio::main]
async fn main() {
    // Define MQTT options
    let mqtt_options = MqttOptions::new("my-client-id", "127.0.0.1", 1883)
        .set_keep_alive(5)
        .set_clean_session(true);

    // Create an MQTT client
    let (mut client, _) = MqttClient::start(mqtt_options).unwrap();

    // Subscribe to a topic
    client.subscribe("bedroom/temperature", QoS::AtMostOnce).unwrap();

    // Publish a message to a topic
    client
        .publish("bedroom/temperature", QoS::AtMostOnce, false, "2".as_bytes())
        .unwrap();

    // Wait for incoming messages (you might want to use a loop here)
    // if let Some(message) = client.incoming().await.next().await {
    //     println!("Received message: {:?}", message);
    // }

    // Disconnect from the broker when done
    // client.disconnect().await.unwrap();
}
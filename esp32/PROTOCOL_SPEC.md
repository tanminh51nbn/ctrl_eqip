# ESP32-C3 Protocol Specification — ctrl_eqip

## Overview

This document describes the binary serial protocol between the **Laptop (AI Node)**
running `ctrl_eqip` (Rust) and the **ESP32-C3 (Edge/Control Node)** firmware.

---

## Serial Settings

| Parameter  | Value        |
|------------|--------------|
| Baud rate  | **115200**   |
| Data bits  | 8            |
| Stop bits  | 1            |
| Parity     | None (8N1)   |
| Connector  | USB (CDC-ACM or CP210x/CH340) |

---

## Frame Format

Every message is wrapped in a frame with the following structure:

```
Byte offset:  0      1        2       3 … (2+LEN)   (3+LEN)   (4+LEN)
              ┌──────┬────────┬───────┬────────────┬─────────┬─────────┐
              │START │ TYPE   │  LEN  │  PAYLOAD   │   CRC8  │   END   │
              │ 0xAA │ 1 byte │ 1 byte│  LEN bytes │  1 byte │  0x55   │
              └──────┴────────┴───────┴────────────┴─────────┴─────────┘
```

| Field   | Size    | Description |
|---------|---------|-------------|
| START   | 1 byte  | Always `0xAA` — frame start marker |
| TYPE    | 1 byte  | Message type ID (see table below) |
| LEN     | 1 byte  | Payload length in bytes (0–250) |
| PAYLOAD | LEN bytes | Message-specific data |
| CRC8    | 1 byte  | CRC-8/SMBUS over TYPE + LEN + PAYLOAD |
| END     | 1 byte  | Always `0x55` — frame end marker |

### CRC-8 algorithm

**CRC-8/SMBUS** — polynomial `0x07`, init `0x00`, no reflection.

```c
// C implementation for ESP32
uint8_t crc8(const uint8_t *data, size_t len) {
    uint8_t crc = 0x00;
    for (size_t i = 0; i < len; i++) {
        crc ^= data[i];
        for (int j = 0; j < 8; j++) {
            if (crc & 0x80) crc = (crc << 1) ^ 0x07;
            else            crc <<= 1;
        }
    }
    return crc;
}
```

CRC is computed over: `{TYPE, LEN, PAYLOAD[0], ..., PAYLOAD[LEN-1]}`

---

## Message Types

### `0x01` — SENSOR_DATA (ESP32 → Laptop)

Sends a temperature reading from the onboard sensor.

| Byte | Content |
|------|---------|
| 0–1  | `temp_raw` — temperature × 100, big-endian `uint16_t` |

**Example**: Temperature = 25.73°C → `temp_raw` = 2573 = `0x0A 0x0D`

**Full frame example for 25.73°C:**
```
AA  01  02  0A 0D  CRC  55
```
Where CRC = crc8({0x01, 0x02, 0x0A, 0x0D})

---

### `0x02` — FAN_COMMAND (Laptop → ESP32)

Sets the fan PWM duty cycle.

| Byte | Content |
|------|---------|
| 0    | `speed` — PWM duty [0–255] where 0=off, 255=full speed |

**Example**: 50% speed → `speed` = 128 = `0x80`

Map to your PWM timer: `ledcWrite(channel, speed);` (ESP32 Arduino)

---

### `0x03` — FAN_OFF (Laptop → ESP32)

Turn the fan off immediately. No payload (LEN = 0).

```
AA  03  00  CRC  55
```

---

### `0x10` — HEARTBEAT (Both directions)

Keep-alive message. Send every 5 seconds from both sides.

| Byte | Content |
|------|---------|
| 0–3  | `uptime_ms` — sender uptime in ms, big-endian `uint32_t` |

---

### `0x11` — ACK (Both directions)

Acknowledge receipt of a message.

| Byte | Content |
|------|---------|
| 0    | `acked_type` — the TYPE byte of the acknowledged message |

---

### `0xFE` — ERROR (Both directions)

Signal an error condition.

| Byte | Content |
|------|---------|
| 0    | `error_code` — see table below |

| Code | Meaning |
|------|---------|
| `0x01` | CRC mismatch in received frame |
| `0x02` | Unknown message type |
| `0x03` | Invalid payload length |
| `0x04` | Sensor read failure (e.g. sensor disconnected) |
| `0xFF` | Unknown/generic error |

---

## Communication Flow

```
Laptop                              ESP32-C3
  │                                    │
  │  ← ← ← SENSOR_DATA (0x01) ← ← ←  │  (every ~500ms)
  │                                    │
  │  → → → FAN_COMMAND (0x02) → → →   │  (after each AI inference cycle ~100ms)
  │    or  FAN_OFF     (0x03)          │
  │                                    │
  │  ← ← ← ACK (0x11) ← ← ← ← ← ← ←  │  (optional, for reliability)
  │                                    │
  │  → → → HEARTBEAT (0x10) → → → →   │  (every 5s, both sides)
  │  ← ← ← HEARTBEAT (0x10) ← ← ← ←  │
```

---

## ESP32-C3 Arduino/IDF Example

### Setup

```c
#include <Arduino.h>

#define BAUD_RATE    115200
#define FAN_PWM_PIN  5        // GPIO connected to fan PWM input
#define PWM_CHANNEL  0
#define PWM_FREQ_HZ  25000    // 25 kHz (inaudible for most fans)
#define PWM_BITS     8        // 8-bit resolution: 0–255

void setup() {
    Serial.begin(BAUD_RATE);
    ledcSetup(PWM_CHANNEL, PWM_FREQ_HZ, PWM_BITS);
    ledcAttachPin(FAN_PWM_PIN, PWM_CHANNEL);
    ledcWrite(PWM_CHANNEL, 0); // fan off at startup
}
```

### Sending SENSOR_DATA

```c
#define FRAME_START 0xAA
#define FRAME_END   0x55

uint8_t crc8(const uint8_t *data, size_t len) { ... } // see above

void send_temperature(float temp_c) {
    uint16_t temp_raw = (uint16_t)(temp_c * 100.0f);
    uint8_t payload[2] = { (temp_raw >> 8) & 0xFF, temp_raw & 0xFF };
    uint8_t type = 0x01;
    uint8_t len  = 2;

    // CRC over: type + len + payload
    uint8_t crc_input[4] = { type, len, payload[0], payload[1] };
    uint8_t crc = crc8(crc_input, 4);

    Serial.write(FRAME_START);
    Serial.write(type);
    Serial.write(len);
    Serial.write(payload, 2);
    Serial.write(crc);
    Serial.write(FRAME_END);
}
```

### Receiving and Parsing Frames

```c
#define MAX_PAYLOAD 250

uint8_t rx_buf[MAX_PAYLOAD + 6];
size_t  rx_pos = 0;
bool    in_frame = false;

void process_frame(uint8_t type, uint8_t *payload, uint8_t len) {
    switch (type) {
        case 0x02: // FAN_COMMAND
            ledcWrite(PWM_CHANNEL, payload[0]);
            break;
        case 0x03: // FAN_OFF
            ledcWrite(PWM_CHANNEL, 0);
            break;
        case 0x10: // HEARTBEAT — optionally echo back
            break;
        default:
            // Send ERROR 0x02 (unknown type)
            break;
    }
}

void loop() {
    // Send temperature every 500ms
    static unsigned long last_send = 0;
    if (millis() - last_send > 500) {
        float temp = read_temperature(); // your sensor read function
        send_temperature(temp);
        last_send = millis();
    }

    // Parse incoming bytes
    while (Serial.available()) {
        uint8_t b = Serial.read();

        if (!in_frame) {
            if (b == FRAME_START) { in_frame = true; rx_pos = 0; }
            continue;
        }

        rx_buf[rx_pos++] = b;

        // Minimum: TYPE(1) + LEN(1) + CRC(1) + END(1) = 4 bytes
        if (rx_pos < 4) continue;

        uint8_t type = rx_buf[0];
        uint8_t len  = rx_buf[1];

        // Wait until full frame is buffered
        if (rx_pos < (size_t)(2 + len + 2)) continue;

        uint8_t rx_crc = rx_buf[2 + len];
        uint8_t end    = rx_buf[3 + len];

        if (end != FRAME_END) { in_frame = false; rx_pos = 0; continue; }

        // Validate CRC
        rx_buf[0] = type; rx_buf[1] = len; // already set
        uint8_t expected_crc = crc8(rx_buf, 2 + len);

        if (rx_crc != expected_crc) {
            // Send CRC error back
            in_frame = false; rx_pos = 0; continue;
        }

        process_frame(type, rx_buf + 2, len);
        in_frame = false; rx_pos = 0;
    }
}
```

---

## Pin Mapping (Reference)

| Signal       | ESP32-C3 GPIO | Notes |
|--------------|---------------|-------|
| UART TX      | GPIO20 (U0TXD) | USB CDC — no extra wiring needed |
| UART RX      | GPIO19 (U0RXD) | USB CDC — no extra wiring needed |
| Fan PWM      | GPIO5 (example) | Connect to fan PWM input |
| Temp Sensor  | GPIO4 (example) | e.g. DS18B20 (1-Wire) or NTC |
| Fan GND      | GND            | Common ground with ESP32 |

> **Note**: When using the built-in USB-CDC on ESP32-C3, `Serial` maps to the USB
> port — no external USB-UART adapter is needed. Plug ESP32 directly into Laptop USB.

---

## Expanding to WiFi (Future)

The Rust side abstracts transport via the `Transport` trait. To switch to WiFi:
1. Implement `Transport` for a TCP or UDP socket in Rust.
2. On ESP32: use `WiFiServer` / `AsyncTCP` to accept connections and send/receive
   the **same binary frame format** (no firmware protocol changes needed).

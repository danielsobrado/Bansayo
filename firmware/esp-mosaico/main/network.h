#pragma once
#include "esp_err.h"
#include "imu_protocol.h"

esp_err_t tracker_network_init(uint8_t mac[6]);
bool tracker_network_ready(void);
int8_t tracker_network_rssi(void);
bool tracker_network_send(const bimu_sample_t *sample);

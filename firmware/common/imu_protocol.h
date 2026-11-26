#pragma once
#include <stdbool.h>
#include <stdint.h>

enum { BIMU_PACKET_SIZE = 64, BIMU_VERSION = 1 };
typedef struct {
    uint8_t mac[6], sensor, status, battery_pct;
    int8_t rssi_dbm;
    uint32_t seq, time_us;
    float q[4], gyro[3], accel[3];
    bool gyro_valid, accel_valid, battery_valid;
} bimu_sample_t;

void bimu_encode(uint8_t out[BIMU_PACKET_SIZE], const bimu_sample_t *sample);

#pragma once
#include <stdbool.h>
#include <stdint.h>

enum { BIMU_PACKET_SIZE = 64, BIMU_VERSION = 1,
       BIMU_V2_PACKET_SIZE = 72, BIMU_VERSION_V2 = 2 };
typedef struct {
    uint8_t mac[6], sensor, status, battery_pct;
    int8_t rssi_dbm;
    uint32_t seq, time_us;
    uint64_t reference_id;
    float q[4], gyro[3], accel[3];
    bool gyro_valid, accel_valid, battery_valid;
} bimu_sample_t;

void bimu_encode(uint8_t out[BIMU_PACKET_SIZE], const bimu_sample_t *sample);
void bimu_encode_v2(uint8_t out[BIMU_V2_PACKET_SIZE], const bimu_sample_t *sample);

enum { BMAG_PACKET_SIZE = 32 };
void bmag_encode(uint8_t out[BMAG_PACKET_SIZE], const uint8_t mac[6],
                 uint8_t sensor, uint8_t index, uint32_t seq,
                 uint32_t time_us, const int16_t values[3]);

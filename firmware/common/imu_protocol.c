#include "imu_protocol.h"
#include <float.h>
#include <string.h>

_Static_assert(sizeof(float) == 4 && FLT_RADIX == 2 && FLT_MANT_DIG == 24,
               "BIMU requires IEEE binary32");

static void put_u32(uint8_t *out, uint32_t value)
{
    for (unsigned i = 0; i < 4; ++i) out[i] = (uint8_t)(value >> (8 * i));
}

static void put_floats(uint8_t *out, const float *values, unsigned n)
{
    for (unsigned i = 0; i < n; ++i) {
        uint32_t bits;
        memcpy(&bits, &values[i], sizeof(bits));
        put_u32(out + 4 * i, bits);
    }
}

void bimu_encode(uint8_t out[BIMU_PACKET_SIZE], const bimu_sample_t *s)
{
    memset(out, 0, BIMU_PACKET_SIZE);
    memcpy(out, "BIMU", 4);
    out[4] = BIMU_VERSION;
    out[5] = (s->gyro_valid ? 1 : 0) | (s->accel_valid ? 2 : 0) |
             (s->battery_valid ? 4 : 0);
    out[6] = s->sensor;
    out[7] = s->status;
    memcpy(out + 8, s->mac, 6);
    out[14] = s->battery_valid ? s->battery_pct : 0;
    out[15] = (uint8_t)s->rssi_dbm;
    put_u32(out + 16, s->seq);
    put_u32(out + 20, s->time_us);
    put_floats(out + 24, s->q, 4);
    if (s->gyro_valid) put_floats(out + 40, s->gyro, 3);
    if (s->accel_valid) put_floats(out + 52, s->accel, 3);
}

void bimu_encode_v2(uint8_t out[BIMU_V2_PACKET_SIZE], const bimu_sample_t *s)
{
    bimu_encode(out, s);
    out[4] = BIMU_VERSION_V2;
    put_u32(out + 64, (uint32_t)s->reference_id);
    put_u32(out + 68, (uint32_t)(s->reference_id >> 32));
}

void bmag_encode(uint8_t out[BMAG_PACKET_SIZE], const uint8_t mac[6],
                 uint8_t sensor, uint8_t index, uint32_t seq,
                 uint32_t time_us, const int16_t values[3])
{
    memset(out, 0, BMAG_PACKET_SIZE);
    memcpy(out, "BMAG", 4);
    out[4] = 1;
    out[5] = sensor;
    out[6] = index;
    memcpy(out + 8, mac, 6);
    put_u32(out + 16, seq);
    put_u32(out + 20, time_us);
    for (unsigned i = 0; i < 3; ++i) {
        const uint16_t bits = (uint16_t)values[i];
        out[24 + 2*i] = (uint8_t)bits;
        out[25 + 2*i] = (uint8_t)(bits >> 8);
    }
}

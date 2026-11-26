#pragma once
#include <stdbool.h>

#define BIMU_GRAVITY_M_S2 9.80665f

typedef struct {
    float q[4]; /* reference_from_sensor, XYZW, +Y up */
    float bias[3];
    float gain;
} ahrs_t;

bool ahrs_init(ahrs_t *state, const float accel[3], const float bias[3], float gain);
bool ahrs_update(ahrs_t *state, const float gyro[3], const float accel[3], float dt);
bool ahrs_stationary(const float gyro[3], const float accel[3]);

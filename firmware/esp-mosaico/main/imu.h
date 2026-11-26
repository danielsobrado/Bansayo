#pragma once
#include "esp_err.h"
esp_err_t tracker_imu_init(void);
esp_err_t tracker_imu_read(float gyro[3], float accel[3]);
void tracker_mag_init(void);
#include <stdbool.h>
#include <stdint.h>
bool tracker_mag_read(unsigned index, int16_t values[3]);

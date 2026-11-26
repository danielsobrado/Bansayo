#pragma once
#include "esp_err.h"
esp_err_t tracker_imu_init(void);
esp_err_t tracker_imu_read(float gyro[3], float accel[3]);
void tracker_mag_init(void);
void tracker_mag_log(void);

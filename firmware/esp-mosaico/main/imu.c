#include "imu.h"
#include "tracker_config.h"
#include "ahrs.h"
#include "bsp/esp_mosaico.h"
#include "esp_check.h"
#include "esp_log.h"

static const char *TAG = "tracker_imu";
static const float radians_per_degree = 0.01745329252f;

esp_err_t tracker_imu_init(void)
{
    ESP_RETURN_ON_ERROR(bsp_imu_init(), TAG, "BMI270 init");
    bsp_imu_config_t config = BSP_IMU_CONFIG_DEFAULT();
    config.accel_range = BMI2_ACC_RANGE_16G;
    config.gyro_range = BMI2_GYR_RANGE_2000;
#ifndef CONFIG_BANSAYO_RATE_100
    config.accel_odr = BMI2_ACC_ODR_200HZ;
    config.gyro_odr = BMI2_GYR_ODR_200HZ;
#endif
    return bsp_imu_start(&config);
}

esp_err_t tracker_imu_read(float gyro[3], float accel[3])
{
    struct bmi2_sens_data data = {0};
    struct bmi2_dev *dev = bsp_imu_get_handle();
    if (!dev || bmi2_get_sensor_data(&data, dev) != BMI2_OK) return ESP_FAIL;
    const float a = 16.0f * BIMU_GRAVITY_M_S2 / 32768.0f;
    const float g = 2000.0f * radians_per_degree / 32768.0f;
    accel[0] = data.acc.x*a; accel[1] = data.acc.y*a; accel[2] = data.acc.z*a;
    gyro[0] = data.gyr.x*g; gyro[1] = data.gyr.y*g; gyro[2] = data.gyr.z*g;
    return ESP_OK;
}

#ifdef CONFIG_BANSAYO_MAG_DIAGNOSTICS
static bool mag_ready[BSP_MAGNETOMETER_NUM];
#endif
void tracker_mag_init(void)
{
#ifdef CONFIG_BANSAYO_MAG_DIAGNOSTICS
    for (unsigned i = 0; i < BSP_MAGNETOMETER_NUM; ++i) {
        mag_ready[i] = bsp_magnetometer_init(i) == ESP_OK;
        if (!mag_ready[i]) ESP_LOGW(TAG, "BMM150 %u unavailable", i);
    }
#endif
}

bool tracker_mag_read(unsigned index, int16_t values[3])
{
#ifdef CONFIG_BANSAYO_MAG_DIAGNOSTICS
    struct bmm150_mag_data m = {0};
    if (index >= BSP_MAGNETOMETER_NUM || !mag_ready[index] ||
        bsp_magnetometer_read(index, &m) != ESP_OK) return false;
    if (m.x == INT16_MIN || m.y == INT16_MIN || m.z == INT16_MIN) return false;
    values[0] = (int16_t)m.x;
    values[1] = (int16_t)m.y;
    values[2] = (int16_t)m.z;
    return true;
#else
    (void)index;
    (void)values;
    return false;
#endif
}

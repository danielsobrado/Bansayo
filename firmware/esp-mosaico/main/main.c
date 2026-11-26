#include "ahrs.h"
#include "display.h"
#include "imu.h"
#include "network.h"
#include "tracker_config.h"
#include "bsp/battery.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "nvs_flash.h"
#include <string.h>

static const char *TAG = "bansayo";

typedef struct {
    unsigned count;
    float accel[3], gyro[3];
} calibration_t;

static bool calibrate(calibration_t *c, ahrs_t *filter, const bimu_sample_t *s)
{
    if (!ahrs_stationary(s->gyro, s->accel)) {
        memset(c, 0, sizeof(*c));
        return false;
    }
    ++c->count;
    for (unsigned i = 0; i < 3; ++i) {
        c->accel[i] += (s->accel[i] - c->accel[i])/c->count;
        c->gyro[i] += (s->gyro[i] - c->gyro[i])/c->count;
    }
    return c->count >= TRACKER_CALIBRATION_SAMPLES &&
        ahrs_init(filter, c->accel, c->gyro, TRACKER_AHRS_GAIN);
}

void app_main(void)
{
    ESP_ERROR_CHECK(nvs_flash_init());
    ESP_ERROR_CHECK(tracker_imu_init());
    tracker_mag_init();
    const bool battery_ready = bsp_battery_init() == ESP_OK;
    if (!battery_ready) ESP_LOGW(TAG, "Battery gauge unavailable");
    tracker_display_init();
    bimu_sample_t sample = {.sensor = TRACKER_SENSOR_NUMBER, .q = {0,0,0,1},
                            .gyro_valid = true, .accel_valid = true};
    const esp_err_t net = tracker_network_init(sample.mac);
    if (net != ESP_OK) {
        ESP_LOGE(TAG, "Network setup failed (%s); check menuconfig", esp_err_to_name(net));
        tracker_display_update("Network configuration error", &sample, 0, 0, 0);
        return;
    }

    ahrs_t filter = {0};
    calibration_t calibration = {0};
    bool calibrated = false;
    uint32_t sent = 0, failed = 0;
    unsigned count = 0, actual_rate = 0, read_errors = 0;
    int64_t previous = 0, last_diag = esp_timer_get_time(), last_ui = 0;
    TickType_t wake = xTaskGetTickCount();
#ifdef CONFIG_BANSAYO_MAG_DIAGNOSTICS
    int64_t last_mag = 0;
    uint32_t mag_seq[2] = {0};
    unsigned mag_index = 0;
#endif
    for (;;) {
        vTaskDelayUntil(&wake, pdMS_TO_TICKS(TRACKER_PERIOD_MS));
        if (tracker_display_recalibrate()) {
            calibrated = false;
            memset(&calibration, 0, sizeof(calibration));
            ESP_LOGI(TAG, "Recalibration requested; hold still");
        }
        const esp_err_t read = tracker_imu_read(sample.gyro, sample.accel);
        const int64_t now = esp_timer_get_time();
        const float dt = (now - previous) / 1000000.0f;
        const char *status = "Keep still: calibrating";
        if (read != ESP_OK) {
            ++read_errors;
            if (!calibrated) memset(&calibration, 0, sizeof(calibration));
            status = "IMU read failed";
        } else {
            previous = now;
            ++count;
            ++sample.seq;
            sample.time_us = (uint32_t)now;
            if (!calibrated) {
                calibrated = calibrate(&calibration, &filter, &sample);
                if (calibrated) ESP_LOGI(TAG, "Initial gyro bias and tilt calibrated");
            } else if (!ahrs_update(&filter, sample.gyro, sample.accel, dt)) {
                calibrated = false;
                memset(&calibration, 0, sizeof(calibration));
                ESP_LOGW(TAG, "IMU timing gap; recalibrating");
            }
            if (calibrated) {
                memcpy(sample.q, filter.q, sizeof(sample.q));
                status = "Wi-Fi disconnected";
                if (tracker_network_ready()) {
                    if (tracker_network_send(&sample)) {
                        ++sent;
                        status = "UDP streaming (unacknowledged)";
                    } else {
                        ++failed;
                        status = "UDP send failed";
                    }
                }
            }
        }
        if (now - last_diag >= TRACKER_DIAGNOSTICS_US) {
            actual_rate = (unsigned)(count * 1000000LL / (now-last_diag));
            count = 0;
            last_diag = now;
            sample.rssi_dbm = tracker_network_rssi();
            bsp_battery_status_t battery;
            sample.battery_valid = battery_ready &&
                bsp_battery_read(&battery) == ESP_OK && battery.state_of_charge <= 100;
            sample.battery_pct = sample.battery_valid ? battery.state_of_charge : 0;
            ESP_LOGI(TAG, "%s rate=%u sent=%lu failed=%lu imu_errors=%u rssi=%d",
                status, actual_rate, (unsigned long)sent, (unsigned long)failed,
                read_errors, (int)sample.rssi_dbm);
        }
#ifdef CONFIG_BANSAYO_MAG_DIAGNOSTICS
        if (now - last_mag >= TRACKER_MAG_INTERVAL_US) {
            last_mag = now;
            int16_t values[3];
            const unsigned index = mag_index;
            mag_index = (mag_index + 1) % 2;
            if (tracker_mag_read(index, values)) {
                const uint32_t sample_time = (uint32_t)esp_timer_get_time();
                ++mag_seq[index];
                (void)tracker_network_mag(sample.mac, index, mag_seq[index], sample_time, values);
            }
        }
#endif
        if (now-last_ui >= TRACKER_DISPLAY_US) {
            last_ui = now;
            tracker_display_update(status, &sample, actual_rate, sent, failed);
        }
        /* Do not emit catch-up bursts after slow peripheral work. */
        const TickType_t current = xTaskGetTickCount();
        if ((TickType_t)(current - wake) >= pdMS_TO_TICKS(TRACKER_PERIOD_MS))
            wake = current;
    }
}

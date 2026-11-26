#include "display.h"
#include "tracker_config.h"
#include "bsp/esp_mosaico.h"
#include "esp_log.h"
#include <stdatomic.h>

static atomic_bool recalibrate;
#if CONFIG_BANSAYO_DISPLAY && CONFIG_BSP_DISPLAY_LVGL_ENABLE
static lv_obj_t *details;
static void on_calibrate(lv_event_t *event)
{
    (void)event;
    atomic_store(&recalibrate, true);
}
#endif

void tracker_display_init(void)
{
#if CONFIG_BANSAYO_DISPLAY && CONFIG_BSP_DISPLAY_LVGL_ENABLE
    if (!bsp_display_start() || !bsp_display_lock(1000)) {
        ESP_LOGW("tracker_ui", "Display unavailable; use serial diagnostics");
        return;
    }
    lv_obj_t *screen = lv_screen_active();
    lv_obj_set_style_bg_color(screen, lv_color_hex(0x14202C), 0);
    lv_obj_t *title = lv_label_create(screen);
    lv_label_set_text(title, "BANSAYO | Mosaico");
    lv_obj_set_style_text_color(title, lv_color_white(), 0);
    lv_obj_align(title, LV_ALIGN_TOP_MID, 0, 28);
    details = lv_label_create(screen);
    lv_obj_set_width(details, 440);
    lv_obj_set_style_text_color(details, lv_color_hex(0xDCE9F4), 0);
    lv_obj_align(details, LV_ALIGN_TOP_LEFT, 20, 80);
    lv_obj_t *button = lv_button_create(screen);
    lv_obj_set_size(button, 260, 64);
    lv_obj_align(button, LV_ALIGN_BOTTOM_MID, 0, -24);
    lv_obj_t *label = lv_label_create(button);
    lv_label_set_text(label, "Recalibrate (keep still)");
    lv_obj_center(label);
    lv_obj_add_event_cb(button, on_calibrate, LV_EVENT_CLICKED, NULL);
    bsp_display_unlock();
#endif
}

bool tracker_display_recalibrate(void)
{
    return atomic_exchange(&recalibrate, false);
}

void tracker_display_update(const char *status, const bimu_sample_t *s,
                            unsigned rate, uint32_t sent, uint32_t dropped)
{
#if CONFIG_BANSAYO_DISPLAY && CONFIG_BSP_DISPLAY_LVGL_ENABLE
    if (!details || !bsp_display_lock(0)) return;
    lv_label_set_text_fmt(details,
        "%s\n\nTarget: %s:%d\nIMU: %u Hz   RSSI: %d dBm\nBattery: %s%u%%\n"
        "UDP sent: %lu   failed: %lu\n\nQ: %.3f %.3f %.3f %.3f\n"
        "Gyro: %.2f %.2f %.2f rad/s\nAccel: %.2f %.2f %.2f m/s2",
        status, CONFIG_BANSAYO_SERVER_IP, CONFIG_BANSAYO_SERVER_PORT,
        rate, (int)s->rssi_dbm, s->battery_valid ? "" : "unknown / ",
        (unsigned)s->battery_pct, (unsigned long)sent, (unsigned long)dropped,
        s->q[0], s->q[1], s->q[2], s->q[3],
        s->gyro[0], s->gyro[1], s->gyro[2], s->accel[0], s->accel[1], s->accel[2]);
    bsp_display_unlock();
#else
    (void)status; (void)s; (void)rate; (void)sent; (void)dropped;
#endif
}

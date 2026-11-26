#include "network.h"
#include "tracker_config.h"
#include "esp_check.h"
#include "esp_event.h"
#include "esp_log.h"
#include "esp_netif.h"
#include "esp_wifi.h"
#include "freertos/FreeRTOS.h"
#include "freertos/event_groups.h"
#include "lwip/sockets.h"
#include <errno.h>
#include <string.h>

static const char *TAG = "tracker_net";
static EventGroupHandle_t events;
static const EventBits_t ip_ready = BIT0;
static struct sockaddr_in destination;
static int udp = -1;

static void on_event(void *arg, esp_event_base_t base, int32_t id, void *data)
{
    (void)arg; (void)data;
    if (base == WIFI_EVENT &&
        (id == WIFI_EVENT_STA_START || id == WIFI_EVENT_STA_DISCONNECTED)) {
        xEventGroupClearBits(events, ip_ready);
        esp_err_t err = esp_wifi_connect();
        if (err != ESP_OK) ESP_LOGW(TAG, "Reconnect: %s", esp_err_to_name(err));
    } else if (base == IP_EVENT && id == IP_EVENT_STA_GOT_IP) {
        xEventGroupSetBits(events, ip_ready);
        ESP_LOGI(TAG, "Wi-Fi has an IP address");
    } else if (base == IP_EVENT && id == IP_EVENT_STA_LOST_IP) {
        xEventGroupClearBits(events, ip_ready);
    }
}

esp_err_t tracker_network_init(uint8_t mac[6])
{
    destination.sin_family = AF_INET;
    destination.sin_port = htons(CONFIG_BANSAYO_SERVER_PORT);
    if (inet_pton(AF_INET, CONFIG_BANSAYO_SERVER_IP, &destination.sin_addr) != 1 ||
        strlen(CONFIG_BANSAYO_WIFI_SSID) == 0)
        return ESP_ERR_INVALID_ARG;
    events = xEventGroupCreate();
    if (!events) return ESP_ERR_NO_MEM;
    ESP_RETURN_ON_ERROR(esp_netif_init(), TAG, "netif");
    ESP_RETURN_ON_ERROR(esp_event_loop_create_default(), TAG, "event loop");
    if (!esp_netif_create_default_wifi_sta()) return ESP_FAIL;
    wifi_init_config_t init = WIFI_INIT_CONFIG_DEFAULT();
    ESP_RETURN_ON_ERROR(esp_wifi_init(&init), TAG, "Wi-Fi init");
    ESP_RETURN_ON_ERROR(esp_wifi_set_storage(WIFI_STORAGE_RAM), TAG, "Wi-Fi storage");
    ESP_RETURN_ON_ERROR(esp_event_handler_register(WIFI_EVENT, ESP_EVENT_ANY_ID, on_event, NULL), TAG, "Wi-Fi events");
    ESP_RETURN_ON_ERROR(esp_event_handler_register(IP_EVENT, ESP_EVENT_ANY_ID, on_event, NULL), TAG, "IP events");
    wifi_config_t config = {0};
    if (strlen(CONFIG_BANSAYO_WIFI_SSID) > sizeof(config.sta.ssid) ||
        strlen(CONFIG_BANSAYO_WIFI_PASSWORD) >= sizeof(config.sta.password))
        return ESP_ERR_INVALID_ARG;
    memcpy(config.sta.ssid, CONFIG_BANSAYO_WIFI_SSID, strlen(CONFIG_BANSAYO_WIFI_SSID));
    memcpy(config.sta.password, CONFIG_BANSAYO_WIFI_PASSWORD, strlen(CONFIG_BANSAYO_WIFI_PASSWORD));
    config.sta.threshold.authmode = WIFI_AUTH_WPA2_PSK;
    ESP_RETURN_ON_ERROR(esp_wifi_set_mode(WIFI_MODE_STA), TAG, "station mode");
    ESP_RETURN_ON_ERROR(esp_wifi_set_config(WIFI_IF_STA, &config), TAG, "station config");
    ESP_RETURN_ON_ERROR(esp_wifi_get_mac(WIFI_IF_STA, mac), TAG, "station MAC");
    ESP_RETURN_ON_ERROR(esp_wifi_start(), TAG, "Wi-Fi start");
    ESP_RETURN_ON_ERROR(esp_wifi_set_ps(WIFI_PS_NONE), TAG, "disable Wi-Fi power save");
    ESP_LOGI(TAG, "Tracker key %02X:%02X:%02X:%02X:%02X:%02X/%u -> %s:%d",
        mac[0], mac[1], mac[2], mac[3], mac[4], mac[5], TRACKER_SENSOR_NUMBER,
        CONFIG_BANSAYO_SERVER_IP, CONFIG_BANSAYO_SERVER_PORT);
    return ESP_OK;
}

bool tracker_network_ready(void)
{
    return events && (xEventGroupGetBits(events) & ip_ready);
}

int8_t tracker_network_rssi(void)
{
    wifi_ap_record_t ap;
    return tracker_network_ready() && esp_wifi_sta_get_ap_info(&ap) == ESP_OK ? ap.rssi : 0;
}

bool tracker_network_send(const bimu_sample_t *sample)
{
    if (!tracker_network_ready()) {
        if (udp >= 0) { close(udp); udp = -1; }
        return false;
    }
    if (udp < 0) {
        udp = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
        if (udp < 0) return false;
    }
    uint8_t packet[BIMU_PACKET_SIZE];
    bimu_encode(packet, sample);
    const int n = sendto(udp, packet, sizeof(packet), MSG_DONTWAIT,
                        (const struct sockaddr *)&destination, sizeof(destination));
    if (n == sizeof(packet)) return true;
    if (errno != EAGAIN && errno != EWOULDBLOCK && errno != ENOBUFS) {
        close(udp);
        udp = -1;
    }
    return false;
}

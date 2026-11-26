idf_component_register(
    SRCS "main.c" "imu.c" "network.c" "display.c"
         "../../common/imu_protocol.c" "../../common/ahrs.c"
    INCLUDE_DIRS "." "../../common"
    PRIV_REQUIRES esp_wifi esp_event esp_netif nvs_flash esp_timer lwip
)

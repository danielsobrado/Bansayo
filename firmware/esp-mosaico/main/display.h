#pragma once
#include <stdbool.h>
#include <stdint.h>
#include "imu_protocol.h"

void tracker_display_init(void);
bool tracker_display_recalibrate(void);
void tracker_display_update(const char *status, const bimu_sample_t *sample,
                            unsigned rate, uint32_t sent, uint32_t dropped);

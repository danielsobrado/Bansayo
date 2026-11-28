#include "ahrs.h"
#include "imu_protocol.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <string.h>

static void test_packet(void)
{
    bimu_sample_t s = {.mac={0xaa,0xbb,0xcc,0xdd,0xee,1}, .sensor=2,
        .seq=0x12345678, .time_us=0x90abcdef, .q={0,0,0,1},
        .gyro={1,-2,0}, .accel={0,0,1}, .gyro_valid=true,
        .accel_valid=true, .battery_valid=true, .battery_pct=77, .rssi_dbm=-58};
    const uint8_t expected[64] = {
        0x42,0x49,0x4d,0x55,1,7,2,0,0xaa,0xbb,0xcc,0xdd,0xee,1,77,198,
        0x78,0x56,0x34,0x12,0xef,0xcd,0xab,0x90,
        0,0,0,0,0,0,0,0,0,0,0,0,0,0,0x80,0x3f,
        0,0,0x80,0x3f,0,0,0,0xc0,0,0,0,0,
        0,0,0,0,0,0,0,0,0,0,0x80,0x3f
    };
    uint8_t b[64];
    bimu_encode(b,&s);
    assert(memcmp(b,expected,sizeof(b))==0);
    s.gyro_valid=s.accel_valid=s.battery_valid=false;
    bimu_encode(b,&s);
    assert(b[5]==0 && b[14]==0);
    for(unsigned i=40;i<64;++i) assert(b[i]==0);
}

static void test_filter(void)
{
    ahrs_t state;
    const float up[3]={0,BIMU_GRAVITY_M_S2,0};
    const float xup[3]={BIMU_GRAVITY_M_S2,0,0};
    const float down[3]={0,-BIMU_GRAVITY_M_S2,0};
    const float zero[3]={0,0,0}, bias[3]={0.01f,0,0};
    assert(ahrs_init(&state,up,bias,1));
    assert(ahrs_stationary(bias,up));
    for(unsigned i=0;i<2000;++i) assert(ahrs_update(&state,bias,up,0.005f));
    assert(fabsf(state.q[3]-1)<1e-5f);
    assert(ahrs_init(&state,xup,zero,1));
    assert(fabsf(state.q[2]-sqrtf(0.5f))<1e-5f);
    assert(ahrs_init(&state,down,zero,1));
    assert(fabsf(state.q[0]-1)<1e-5f);
    assert(!ahrs_init(&state,zero,zero,1));
    assert(ahrs_init(&state,up,zero,0));
    const float yaw[3]={0,1.5707963268f,0};
    for(unsigned i=0;i<200;++i) assert(ahrs_update(&state,yaw,up,0.005f));
    assert(fabsf(state.q[1]-sqrtf(0.5f))<1e-4f);
    assert(fabsf(state.q[3]-sqrtf(0.5f))<1e-4f);
    assert(!ahrs_update(&state,yaw,up,1));
    assert(!ahrs_update(&state,yaw,up,NAN));
    assert(!ahrs_stationary(yaw,up));
}

static void test_magnetic_packet(void)
{
    const uint8_t mac[6] = {1,2,3,4,5,6};
    const int16_t values[3] = {-100,0,32767};
    uint8_t b[BMAG_PACKET_SIZE];
    bmag_encode(b,mac,0,1,42,0x12345678,values);
    const uint8_t expected[32] = {
        0x42,0x4d,0x41,0x47,1,0,1,0,1,2,3,4,5,6,0,0,
        42,0,0,0,0x78,0x56,0x34,0x12,0x9c,0xff,0,0,0xff,0x7f,0,0
    };
    assert(memcmp(b,expected,sizeof(b))==0);
    uint8_t v2[BMAG_V2_PACKET_SIZE];
    const uint8_t reference[8] = {0xef,0xcd,0xab,0x90,0x78,0x56,0x34,0x12};
    bmag_encode_v2(v2,mac,0,1,42,0x12345678,values,UINT64_C(0x1234567890abcdef));
    assert(v2[4]==BMAG_VERSION_V2 && memcmp(v2+32,reference,sizeof(reference))==0);
    v2[4]=1;
    assert(memcmp(v2,expected,sizeof(expected))==0);
}

int main(void)
{
    test_magnetic_packet();
    bimu_sample_t sample = {.reference_id=UINT64_C(0x1234567890abcdef),.q={0,0,0,1}};
    uint8_t v2[BIMU_V2_PACKET_SIZE];
    bimu_encode_v2(v2,&sample);
    const uint8_t reference[8]={0xef,0xcd,0xab,0x90,0x78,0x56,0x34,0x12};
    assert(v2[4]==2 && memcmp(v2+64,reference,sizeof(reference))==0);
    test_packet();
    test_filter();
    puts("BIMU protocol and AHRS tests passed");
    return 0;
}

// SPDX-License-Identifier: GPL-3.0-or-later
//
// The console side of NHSE for Switch 2: libounce (libnx for Switch 2) calls for the
// framebuffer, buttons, the software keyboard, user accounts and the game's save data.
// The app itself is Rust (crates/nhse-switch, linked as libnhse_switch.a); it calls the
// nhse_* functions below (see crates/nhse-switch/src/nx.rs).
#include <stdint.h>
#include <dirent.h>
#include <errno.h>
#include <limits.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>
#include <sys/types.h>
#include <time.h>

#include <switch.h>

#define WIDTH 1280
#define HEIGHT 720
#define SAVE_MOUNT "acnh"

int nhse_main(void);

static Framebuffer g_fb;
static PadState g_pad;
static bool g_fb_ready;
static bool g_save_mounted;

// Button bits shared with crates/nhse-switch/src/platform.rs (mod btn).
enum {
    BTN_A = 1 << 0,
    BTN_B = 1 << 1,
    BTN_X = 1 << 2,
    BTN_Y = 1 << 3,
    BTN_L = 1 << 4,
    BTN_R = 1 << 5,
    BTN_ZL = 1 << 6,
    BTN_ZR = 1 << 7,
    BTN_PLUS = 1 << 8,
    BTN_MINUS = 1 << 9,
    BTN_UP = 1 << 10,
    BTN_DOWN = 1 << 11,
    BTN_LEFT = 1 << 12,
    BTN_RIGHT = 1 << 13,
};

static uint32_t map_buttons(u64 b)
{
    uint32_t o = 0;
    if (b & HidNpadButton_A) o |= BTN_A;
    if (b & HidNpadButton_B) o |= BTN_B;
    if (b & HidNpadButton_X) o |= BTN_X;
    if (b & HidNpadButton_Y) o |= BTN_Y;
    if (b & HidNpadButton_L) o |= BTN_L;
    if (b & HidNpadButton_R) o |= BTN_R;
    if (b & HidNpadButton_ZL) o |= BTN_ZL;
    if (b & HidNpadButton_ZR) o |= BTN_ZR;
    if (b & HidNpadButton_Plus) o |= BTN_PLUS;
    if (b & HidNpadButton_Minus) o |= BTN_MINUS;
    // the D-pad and the left stick move the same way
    if (b & (HidNpadButton_Up | HidNpadButton_StickLUp)) o |= BTN_UP;
    if (b & (HidNpadButton_Down | HidNpadButton_StickLDown)) o |= BTN_DOWN;
    if (b & (HidNpadButton_Left | HidNpadButton_StickLLeft)) o |= BTN_LEFT;
    if (b & (HidNpadButton_Right | HidNpadButton_StickLRight)) o |= BTN_RIGHT;
    return o;
}

// Waits for the next frame. Returns 0 when the app must close.
int nhse_frame(uint32_t *down, uint32_t *held)
{
    if (!appletMainLoop())
        return 0;
    padUpdate(&g_pad);
    *down = map_buttons(padGetButtonsDown(&g_pad));
    *held = map_buttons(padGetButtons(&g_pad));
    return 1;
}

// Shows a frame: `px` holds width*height pixels, R G B A bytes each.
// framebufferBegin waits for the display, which paces the app at its refresh rate.
void nhse_present(const uint32_t *px, uint32_t width, uint32_t height)
{
    if (!g_fb_ready)
        return;
    u32 stride = 0;
    uint8_t *dst = framebufferBegin(&g_fb, &stride);
    if (dst) {
        for (uint32_t y = 0; y < height && y < HEIGHT; y++)
            memcpy(dst + (size_t)y * stride, px + (size_t)y * width,
                   (width < WIDTH ? width : WIDTH) * 4);
    }
    framebufferEnd(&g_fb);
}

// Shows the software keyboard. Returns 1 with the text in `out`, 0 when cancelled.
int nhse_keyboard(const char *header, const char *guide, const char *initial,
                  int numeric, int max_len, char *out, size_t out_size)
{
    SwkbdConfig kbd;
    if (R_FAILED(swkbdCreate(&kbd, 0)))
        return -1;
    swkbdConfigMakePresetDefault(&kbd);
    if (numeric)
        swkbdConfigSetType(&kbd, SwkbdType_NumPad);
    swkbdConfigSetHeaderText(&kbd, header);
    swkbdConfigSetGuideText(&kbd, guide);
    swkbdConfigSetInitialText(&kbd, initial);
    if (max_len > 0)
        swkbdConfigSetStringLenMax(&kbd, (u32)max_len);
    memset(out, 0, out_size);
    Result rc = swkbdShow(&kbd, out, out_size);
    swkbdClose(&kbd);
    return R_SUCCEEDED(rc) ? 1 : 0;
}

// Lists the console's users: 16-byte ids into `uids`, nicknames (33 bytes each) into `names`.
// Returns the count, or -1 with the result code in `rc_out`.
int nhse_users(uint8_t *uids, char *names, int max, uint32_t *rc_out)
{
    Result rc = accountInitialize(AccountServiceType_Application);
    if (R_FAILED(rc)) {
        *rc_out = rc;
        return -1;
    }
    AccountUid list[ACC_USER_LIST_SIZE];
    s32 total = 0;
    if (max > ACC_USER_LIST_SIZE)
        max = ACC_USER_LIST_SIZE;
    rc = accountListAllUsers(list, max, &total);
    if (R_FAILED(rc)) {
        accountExit();
        *rc_out = rc;
        return -1;
    }
    for (s32 i = 0; i < total && i < max; i++) {
        memcpy(uids + i * 16, &list[i], 16);
        char *name = names + i * 33;
        snprintf(name, 33, "User %d", (int)i + 1);
        AccountProfile profile;
        if (R_SUCCEEDED(accountGetProfile(&profile, list[i]))) {
            AccountProfileBase base;
            if (R_SUCCEEDED(accountProfileGet(&profile, NULL, &base))) {
                memcpy(name, base.nickname, 32);
                name[32] = 0;
            }
            accountProfileClose(&profile);
        }
    }
    accountExit();
    return total < max ? total : max;
}

// Mounts Animal Crossing's console-wide island save as "acnh:/". Returns 0 or a result code.
uint32_t nhse_mount_save(uint64_t title_id)
{
    if (g_save_mounted) {
        fsdevUnmountDevice(SAVE_MOUNT);
        g_save_mounted = false;
    }
    Result rc = fsdevMountDeviceSaveData(SAVE_MOUNT, title_id);
    g_save_mounted = R_SUCCEEDED(rc);
    return rc;
}

// Rust's Horizon stat/dirent layout differs from newlib's; keep type checks in C.
int nhse_path_kind(const char *path)
{
    struct stat st;
    if (stat(path, &st) != 0)
        return 0;
    return S_ISREG(st.st_mode) ? 1 : S_ISDIR(st.st_mode) ? 2 : 0;
}

int nhse_path_size(const char *path, uint64_t *size)
{
    struct stat st;
    if (stat(path, &st) != 0)
        return -errno;
    *size = (uint64_t)st.st_size;
    return 0;
}

// Return the size needed for type/name pairs. Rust uses the wrong Horizon dirent layout.
int nhse_list_save_dir(const char *path, char *out, size_t capacity)
{
    DIR *dir = opendir(path);
    if (!dir)
        return -errno;
    size_t used = 0;
    struct dirent *entry;
    for (;;) {
        errno = 0;
        entry = readdir(dir);
        if (!entry) break;
        if (!strcmp(entry->d_name, ".") || !strcmp(entry->d_name, ".."))
            continue;
        size_t len = strlen(entry->d_name) + 1;
        if (used > INT_MAX - 1 - len) {
            closedir(dir);
            return -EOVERFLOW;
        }
        if (used + 1 + len > capacity) {
            used += 1 + len;
            continue;
        }
        char full[PATH_MAX];
        int n = snprintf(full, sizeof full, "%s%s%s", path, path[strlen(path) - 1] == '/' ? "" : "/", entry->d_name);
        if (n < 0 || (size_t)n >= sizeof full) {
            closedir(dir);
            return -ENAMETOOLONG;
        }
        struct stat st;
        if (stat(full, &st) != 0) {
            int err = errno;
            closedir(dir);
            return -err;
        }
        out[used++] = S_ISDIR(st.st_mode) ? 1 : 0;
        memcpy(out + used, entry->d_name, len);
        used += len;
    }
    int err = errno;
    closedir(dir);
    return err ? -err : (int)used;
}

// Delete only NHSE's SD working copy. C directory entries retain their full names on Horizon.
static int remove_work_tree(const char *path)
{
    DIR *dir = opendir(path);
    if (!dir)
        return errno == ENOENT ? 0 : -errno;
    struct dirent *entry;
    while ((entry = readdir(dir))) {
        if (!strcmp(entry->d_name, ".") || !strcmp(entry->d_name, ".."))
            continue;
        char full[512];
        int n = snprintf(full, sizeof full, "%s/%s", path, entry->d_name);
        if (n < 0 || (size_t)n >= sizeof full) {
            closedir(dir);
            return -ENAMETOOLONG;
        }
        struct stat st;
        if (stat(full, &st) != 0) {
            int err = errno;
            closedir(dir);
            return -err;
        }
        int rc = S_ISDIR(st.st_mode) ? remove_work_tree(full) : unlink(full) == 0 ? 0 : -errno;
        if (rc != 0) {
            closedir(dir);
            return rc;
        }
    }
    closedir(dir);
    return rmdir(path) == 0 ? 0 : -errno;
}

int nhse_clear_work_dir(const char *path)
{
    // Only a 32-digit account work folder may be removed.
    const char *prefix = "sdmc:/switch/nhse/game/";
    size_t prefix_len = strlen(prefix);
    if (strncmp(path, prefix, prefix_len) != 0 || strlen(path + prefix_len) != 32)
        return -EINVAL;
    for (const char *p = path + prefix_len; *p; p++)
        if (!((*p >= '0' && *p <= '9') || (*p >= 'a' && *p <= 'f')))
            return -EINVAL;
    return remove_work_tree(path);
}

int nhse_clear_backup_dir(const char *path)
{
    const char *prefix = "sdmc:/switch/nhse/backups/";
    size_t prefix_len = strlen(prefix);
    if (strncmp(path, prefix, prefix_len) != 0)
        return -EINVAL;
    const char *name = path + prefix_len;
    if (!*name || strchr(name, '/') || !strcmp(name, ".") || !strcmp(name, ".."))
        return -EINVAL;
    return remove_work_tree(path);
}

// Commits the writes made to the mounted save data, all at once.
uint32_t nhse_commit_save(void)
{
    return g_save_mounted ? fsdevCommitDevice(SAVE_MOUNT) : MAKERESULT(Module_Libnx, LibnxError_NotInitialized);
}

// Unmounts the save data; writes that were not committed are dropped.
void nhse_unmount_save(void)
{
    if (g_save_mounted)
        fsdevUnmountDevice(SAVE_MOUNT);
    g_save_mounted = false;
}

// Seconds since 1970 in the console's time zone.
int64_t nhse_local_time(void)
{
    u64 ts = 0;
    if (R_FAILED(timeGetCurrentTime(TimeType_LocalSystemClock, &ts)))
        return (int64_t)time(NULL);
    TimeCalendarTime cal;
    TimeCalendarAdditionalInfo info;
    if (R_SUCCEEDED(timeToCalendarTimeWithMyRule(ts, &cal, &info)))
        return (int64_t)ts + info.offset;
    return (int64_t)ts;
}

// The console's theme: 1 dark, 0 light, -1 unknown.
int nhse_system_dark(void)
{
    if (R_FAILED(setsysInitialize()))
        return -1;
    ColorSetId id = ColorSetId_Light;
    Result rc = setsysGetColorSetId(&id);
    setsysExit();
    if (R_FAILED(rc))
        return -1;
    return id == ColorSetId_Dark ? 1 : 0;
}

// Rust's standard library seeds its hash maps with getrandom, which newlib lacks.
ssize_t getrandom(void *buf, size_t len, unsigned int flags)
{
    (void)flags;
    randomGet(buf, len);
    return (ssize_t)len;
}

int main(int argc, char **argv)
{
    (void)argc;
    (void)argv;
    padConfigureInput(1, HidNpadStyleSet_NpadStandard);
    padInitializeDefault(&g_pad);

    if (R_SUCCEEDED(framebufferCreate(&g_fb, nwindowGetDefault(), WIDTH, HEIGHT,
                                      PIXEL_FORMAT_RGBA_8888, 2))) {
        if (R_SUCCEEDED(framebufferMakeLinear(&g_fb)))
            g_fb_ready = true;
        else
            framebufferClose(&g_fb);
    }
    if (!g_fb_ready) {
        // without a framebuffer, say so on the text console
        consoleInit(NULL);
        printf("NHSE could not open the display.\nPress + to exit.\n");
        while (appletMainLoop()) {
            padUpdate(&g_pad);
            if (padGetButtonsDown(&g_pad) & HidNpadButton_Plus)
                break;
            consoleUpdate(NULL);
        }
        consoleExit(NULL);
        return 1;
    }

    int ret = nhse_main();

    nhse_unmount_save();
    framebufferClose(&g_fb);
    return ret;
}

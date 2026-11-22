# 一加 Turbo 6（`op60f0l1` / PLU110）Skyward 移植

本仓库是 [kirklin/skyward](https://github.com/kirklin/skyward) 针对**一加 Turbo 6（PLU110）**的移植分支，
仓库名 `MingZhi-awa/skyward-for-oneplusturbo6`。它不是一加或上游 Skyward 的官方发布。

- 设备内部代号：`op60f0l1`（`ro.product.device`，`oplusboot.prjname` = 24816），平台 `sun`（SM8735，骁龙 8s Gen 4）
- 市场型号：OnePlus Turbo 6 / PLU110；**不是** Ace 6（PLQ110 / `ossi` / SM8750），也**不是** Ace 6T（PLR110）
- 测试系统：ColorOS **V16.1.0** / PLU110_16.0.9.405(CN01) / Android 16，A 槽跑系统，B 槽无系统
- recovery 分区原为 TWRP，原厂镜像本身没有备份（后从固件里取出，见下）

## 三个仓库的关系

```
kirklin/skyward                                 上游，Rust 写的 recovery，GPL-3.0
  └─ liuyimeng1024-ui/skyward-for-oneplusace6   一加 Ace 6 移植（`ossi` / PLQ110）
       └─ MingZhi-awa/skyward-for-oneplusturbo6 本仓库（本机）
```

- 本机 fork 自 Ace 6 移植仓库，`git remote` 里 `origin` 写的是 `...oneplusace6.git`，push 时 GitHub 会重定向并给提示
- 基线提交：上游 `9b1054b`（Ace 6 移植的 v0.3.1 用的就是它）
- 与 Ace 6 移植**不能互相刷**：两台机器的 SoC 不同（SM8735 / SM8750），Wi-Fi 芯片不同（WCN7750 / kiwi_v2），
  编解码器不同（wcd9378 / wcd939x），remote processor 编号也不同

## 当前状态

| 功能 | 说明 |
| --- | --- |
| 显示 | 1272×2772 @165Hz（面板 AA610_P_3_A0034_dsc_vid，BOE，density 560，圆角 143） |
| 亮度 | `oplus,dsi-brightness-normal-max-level` 3515 / `qcom,mdss-brightness-max-level` 4094 |
| 触屏 | Synaptics S3910，走 OPLUS hbp 层，`/dev/hbp0` 上 ioctl `0xC50A` 使能中断 |
| 音量键导航 | 上游自带 |
| adb | root，无需手机端授权 |
| 分区挂载 | super 21 个逻辑分区，modem、persist 可挂 |
| data 解密 | 未完整验证（key service 走手机自己的 init 脚本启动） |
| Wi-Fi | 能起来并连接，但慢且有历史坑（见「已知问题」） |
| 机型显示 | OnePlus Turbo 6 / 骁龙 8s Gen 4 |
| OS 版本显示 | 读 `ro.build.version.oplusrom`（`/my_product/build.prop`） |

未验证：OTA 安装、双系统、MTP 文件传输的读写细节、振动。

## 刷入

只刷 `recovery` 分区，不改 boot / init_boot / vendor_boot。

```sh
fastboot flash recovery skyward-plu110-<版本>.img
fastboot reboot recovery
```

回滚到原厂：

```sh
fastboot flash recovery recovery_PLU110_Android16.img
```

- 原厂镜像 100 MiB，SHA-256 `cb93d62024f7b26cccf31232fdf8c26c546f566f8556ed9e14c48f8b337d377b`，
  取自 PLU110_16.0.9.405(CN01) 的 `recovery_a`，作为 release `stock-plu110` 的资产存放
- 镜像的 AVB footer 未签名，只有解锁状态能启动，**不要重新锁 BL**
- 完整的发版与刷入流程（Actions 编译、tag、镜像站下载、dd、镜像体检）另见记忆里的「发版流程」，这里只记结论：
  打 tag 不一定触发 workflow，要用 API 手动派发到 tag ref

## 本移植相对上游的改动

按性质分三类，方便下次只挑需要的那一段。

### 一、通用修复（对其它 OPLUS 机型也成立）

1. **面板节点查找加回退**（`crates/skyward/src/ui.rs`，提交 `eff2ef2`）
   上游只在 `/proc/device-tree/soc/<面板名>` 下找亮度的 normal max level。OPLUS 的内核把节点放在
   `/proc/device-tree/soc/qcom,mdss_mdp@ae00000/<面板名>`，并在 `soc/` 下留一个同名的空壳节点，
   于是查到一个没有任何属性的节点就放弃了。现在两处都找，近的优先。

2. **触屏日志级别调低**（`crates/skyward/src/input.rs`，随上游 Ace 6 v0.3.2 合入）
   hbp 驱动默认 INFO：每报一个触摸点就写一行 hex 加一行坐标，手指一划每秒几千行，log ring 填满、
   写日志的进程抢 CPU、机器卡死。现在使能中断的同时用 `0xC514` 把级别压到 FATAL。
   **这条对所有走 hbp 的 OPLUS 机型都成立**。

3. **密钥程序不再在设备配置里重复列出**（`devices/op60f0l1.toml`，随上游合入）
   recovery 本来就会读手机自己的 init 脚本，按脚本里写的顺序和参数起 qseecomd、QSEECom service、
   key service、credential service、boot service。配置里再列一遍就是各起两份，两套 key/credential
   service 会互相抢安全世界的回答，解密页就会一直等一个不来的回应。**本机配置已删掉这四条**。

### 二、本机专有

4. **WLAN 走 icnss2，没有 `fs_ready`**（`devices/op60f0l1.toml`，提交 `b579c52`）
   见「已知问题 1」。

5. **Wi-Fi 密码保存**（`crates/skyward/src/saved_wlan.rs` 新增，提交 `8519963`）
   输过一次密码的网络，下次点一下就加入：密码写到 `persist`（recovery 里唯一能在任何用户数据解密前
   写的地方），一行一个网络，名字和密码都按 hex 存，这样空格或任何文本装不下的字节都不会破坏行结构。
   密码在该 AP 已经接受时就算保留（早于拿到 IP），所以“连上了但没给地址”的网络也记得住；忘记网络会一并删掉。
   细节页会说密码是谁的，WLAN 开关关掉时不再顺手清空已保存的网络。
   **注意**：存在 `persist` 意味着恢复出厂也还在，拿到手机的人读得到这些密码；文件权限 0600，仅此而已。

6. **设备配置**（`devices/op60f0l1.toml`，提交 `ba09e91`）
   全部取自实机，不是猜的：
   - `codename = "op60f0l1"`：备份清单会记这个名字，恢复时拒绝别的机器的备份，
     所以一旦定了就不要改（Ace 6 那边改成 PLQ110 是为了这个名字统一，本机一直用 `op60f0l1`，不改）
   - `sources = ["vendor_boot", "vendor_dlkm", "vendor", "system_dlkm"]`：`rfkill.ko` 在 `system_dlkm`
   - 没有 `[[owners]]`：本机 NFC 走 TMS，内核只造 `/dev/tms_nfc`，`/dev/nq-nci` 根本不存在；
     等一个不来的节点会让整条 bring-up 卡死五秒后中止
   - 振动：`qcom-hv-haptics` 能加载，但本机没有 `/sys/class/qcom-haptics` 节点可读周期，
     只有 `oplus_haptic` 的 `haptic_fb_class`，所以用移植的默认值，暂不配置 `[haptics]`
   - `[backup].efs` 加了 `oplusdycnvbk`（本机有，`oplusstanvbk` 按槽保留，不取）

7. **CI 改动**（`4fb287c`、`ba09e91`）
   原厂 recovery 不从一加服务器下（stock 命令要一个带版本号的完整更新包 URL，这里没有），
   改成从本仓库 release `stock-plu110` 的资产取，并按 SHA-256 校验后再打包。
   产物上传拆成两步：一个 artifact 带 `archive: false` 只能装一个文件，原来一次给两个，打包完就崩了。

### 三、给上游 Ace 6 移植带过来的行为

8. **USB 口一次只挂一种功能**（`crates/skyward-sys/src/usb.rs`、`crates/skyward-pack/src/mtp.rc`、
   `crates/skyward/src/ui.rs`）
   文件传输和 adb 不再同时出现，设置页的开关选一个：开是文件传输，关是 adb。
   服务本身无论开关都起（起服务才会挂上 MTP function），开关只决定口上挂哪种配置。
   本机配置写 `mtp = false`，也就是开机就是 adb——毕竟 recovery 主要靠 adb 驱动，
   而“两个功能同时在会不会重置端口”在本机还没实测过。

9. **`mtp` 字段重新接上线**（`crates/skyward/src/ui.rs`，本仓库自己的两行改动）
   上游 v0.3.2 之后 `cfg.device.mtp` 其实没有任何代码读了，开关的初值是硬编码的 `true`，
   于是配置里写 `false` 不生效、开机就是文件传输模式、adb 反而没了。现在开机时用配置的值，
   而设置里存下的选择仍然优先。

10. **电量改从 PMIC 的 `vbat_sns` 读**（`crates/skyward-info/src/lib.rs`）
    本机的 `oplus_chg_v2` 在 recovery 里建不出 item table，`power_supply` 是空的，没有 fuel gauge 可读。
    现在先问 fuel gauge，问不到就在 `soc` 下按名字松散匹配 `qcom,spmi` 总线，最多往下六层找
    `in_voltage_pmih010x_vbat_sns_qbg_input`（微伏），用锂电曲线内插成百分比。
    **精度约 ±10%**，满电附近变化不明显；充电器在不在读不出来，一律 `Unknown`。

11. **刷包 pre-device 支持逗号分隔**（`crates/skyward-install/src/package.rs`）
    一加整包写 `OP6113L1,PLQ110`，只按 `|` 切的话会当成一个名字，然后拒绝本该能刷的包。

12. **安装进度按实际读到的字节计数**（`crates/skyward-install/src/fuse.rs`）
    以前一个 payload 块可能被计两次，进度条会跳。

13. **data 分区可以不带 key service 打开**（`crates/skyward-crypto/`、`decrypt.rs`、`wipe.rs`）
    分区表里根本没加密的 data 不再需要先连 key service；被系统一直保持未加密的 data 也有对应的重置路径。

## 本次合并的上游更新（2026-11-22）

### 一加 Ace 6 移植：两次 release

`v0.3.1-ossi`（第一个可用版本）→ 已包含在 fork 里，本机的改动就是站在它上面的。

`v0.3.2-ossi`（第一个稳定版）的四件事，落到本机的处理：

| 上游内容 | 本机处理 |
| --- | --- |
| USB 端口 file transfer / adb 二选一，设置页开关 | 采用，配置 `mtp = false`，并补上 `mtp` 字段没被读的漏洞 |
| 电量从 PMIC `vbat_sns` 换算（±10%） | 采用，路径按名字松散查找，不写死地址 |
| 不启 modem（WLAN 不需要，启了在 Ace 6 上 48 秒内核卡死） | **不采用**。本机 WLAN 靠 icnss2 + 启 wpss，modem（remoteproc2）仍在启动列表里；若本机出现约 48 秒卡死，先按上游删掉 `remoteproc2` 那行再测 |
| USB 口自有模块交给手机（wcd_usbss_i2c 每 15 秒对端口做 surge 检查） | **采用**，从 `modules.load` 删掉了 wcd9378_dlkm / wcd_usbss_i2c / fsa4480-i2c / redriver / repeater / repeater-qti-pmic-eusb2；配置注释里留了原样五行，端口不工作就放回去 |
| pre-device 支持逗号分隔 | 采用 |
| codename 改成机器型号（PLQ110） | 不采用，本机一直叫 `op60f0l1`，改了会让旧备份恢复不了 |

`ab910de`（README / 文档加“这是修改版的 Skyward”）只动文档，没有合。

### 上游 kirklin/skyward：`9b1054b` 之后 5 个提交

全部是通用修复，直接 merge 进来，无冲突：

```
7950878 fix(install): count the payload's bytes as the update's progress (#63)
4086312 fix(install): count a block of the payload once, as it is first read
3857f4c feat(crypto): open a data partition the phone keeps unencrypted (#62)
7568a50 feat(crypto): open a plain data partition without the key service
dc7f769 fix: reset a data partition the phone keeps unencrypted
```

注意上游这 5 个提交都还没发 release（最新 release 仍是 v0.3.1），也就是本仓库现在跑的是上游 main 的最新状态。

## 已知问题（含实测日志）

### 1. WLAN：没有 `fs_ready`（已修）

```
[   35.958567] the WLAN did not come up: write 1 to /sys/kernel/cnss/fs_ready: No such file or directory (os error 2)
```

上游写 `/sys/kernel/cnss/fs_ready` 是 cnss2 的路子。本机是 **icnss2**，压根没有这个节点：
芯片靠启 wpss 来带起，手机自己的 `/vendor/etc/init/init.vendor.wlan.rc` 在 early-boot 里就是干这个的：

```
write /sys/kernel/icnss/wlan_en_delay 1000
write /sys/kernel/icnss/wpss_boot 1
```

现在配置的 `[wlan].writes` 里写这两行。另外本机 remote processor 编号是 adsp 0 / cdsp 1 / mss 2 / wpss 3，
modem 是 `remoteproc2`（Ace 6 那边不是这个编号）。

### 2. WLAN：wlan0 起不来，`Device or resource busy`（未修完）

```
[   59.833643] the WLAN did not come up: bring wlan0 up: Device or resource busy (os error 16)
```

指向 qcacld 的 `hdd_open` 被拒，怀疑是 `DEVICE_OPEN` 标志没清，或者 cnss 平台时序不对。
实测的缓解办法：**进 recovery 之后手动把 Wi-Fi 开关关一次再开**，能起来（所以下一步是让它自己在启动时做一次
down/up，或者干脆先 down 再 up）。

### 3. WLAN：起来很慢

从开机到能扫描约 60-90 秒（Ace 6 实测模块加载在 64.9 秒）。本机同样要等 modem 固件加载 + cnss 校准 +
四十多个依赖模块。打开 Wi-Fi 页面后要等，不是卡死。

### 4. USB 口的行为在本机没实测

“file transfer + adb 同时在会不会重置端口”是 Ace 6 上的结论，本机直接沿用了保守做法（默认 adb）。
如果实测发现本机根本不在乎，可以把 `mtp` 改回 `true`，行为退回“开机就挂文件传输”。

### 5. 振动

`qcom-hv-haptics` 加载了但取不到周期（只有 `haptic_fb_class`），用默认值。要么找到
`oplus_haptic` 暴露周期的节点，要么从别的 OPLUS 机型借一个值。

## 下次适配的检查清单

合上游的时候按这个顺序过一遍，能少踩一半的坑。

1. **先看上游有没有动设备配置的字段语义**。Ace 6 的 `v0.3.2` 就把 `mtp` 从“要不要挂文件传输”变成了
   “开关初值”，而且代码里其实没人读——只看 diff 看不出来，得 grep 一遍字段名。
2. **`devices/` 里的文件名会互相踩**。上游把 `ossi.toml` 改名成 `PLQ110.toml`，cherry-pick 过来会
   凭空多一个别人的设备配置；要么留下（只当参考，CI 只编 `$DEVICE`），要么删掉，但别让 config 的测试去
   `include_str!` 一个不存在的文件。
3. **别整分支 merge Ace 6 的移植仓库**。它的提交里混着“删掉本机的 op60f0l1.toml、删掉 saved_wlan.rs”
   这种反向改动，`git diff HEAD ace6up` 会看起来很吓人。用 `cherry-pick -n` 只挑代码，设备配置自己判断。
4. **每加一条设备配置的“为什么”，都写下证据**（哪个节点、哪行日志、哪个 ioctl），下次才有得对。
5. **本地先验一遍**：
   ```sh
   cargo test -p skyward-config -p skyward-ui -p skyward-install -p skyward-crypto
   export CC_aarch64_linux_android=aarch64-linux-gnu-gcc
   export AR_aarch64_linux_android=aarch64-linux-gnu-ar
   cargo check -p skyward --target aarch64-linux-android
   ```
   `#[cfg(target_os = "android")]` 底下那些模块（wlan、saved_wlan、input）只有交叉检查才编得到。
6. **镜像体检**（确认新代码真进去了）：大小 104857600、头 8 字节 `ANDROID!`、header 1584、
   ramdisk 在 `4096 + kernel_size`、magic `02 21 4c 18`（lz4 legacy），`lz4 -d | cpio -i --to-stdout
   system/bin/recovery` 抠出二进制后 `grep -a` 找新加的字符串。

## Ace 6 `3909c9c`：键盘改成独立 dock 之后

Ace 6 的 `fix(ui): hold the keyboard at the foot of the page` 把键盘从“画在页面上”改成
`Frame.dock`（自己的画布，贴在屏幕底部，页面在它下面滚动），并把收更新地址换成了他们自己的仓库。
合过来之后有两处必须跟着改，否则搜索页会直接 panic（`put_rows` 的 “the rows put are held”）：

1. **`keyboard_strip` 要认 dock**。它原来是把页面底部那几行键盘从页面画布里拷出来，给搜索栏
   上滑/回落动画用；键盘搬进 dock、页面又变成可滚动的长页之后，页面底部那几行根本不在
   画布里（高度 4616 的画布只画了 0..3200）。现在有 dock 就直接拷 dock 的画布。
2. **打字只重画键盘**。`input.rs` 里找“按下/抬起的那颗键”的矩形只在页面的 `hits` 里找，
   dock 上的键找不到就退化成整页重画。改成：页面里没有就在 `frame.dock.hits` 里找，
   命中则走新的 `refresh_dock()`——下一帧只重画 dock，页面一个像素都不动。
3. **页面高度会随内容变**。搜索页现在是长页，结果条数一变高度就变（空搜索 3200，输一个
   `b` 变 4616），这时候只重画几行是画不对的。`render()` 里比一下新旧高度，不一致就整页重来。
4. 更新的第 2、3 条和 `a_page_typed_on...`、`the_cursor_blinks...` 两个测试的旧假设对不上，
   测试改成核对“画布已画出的那些行”与整页重画是否一致，以及“按下键时重画的是 dock 或那几行”。

顺带把 `github.rs` 的 `LATEST` 从 Ace 6 的仓库改回本仓库
（`MingZhi-awa/skyward-for-oneplusturbo6`）：更新页要查的是本机自己的 release，查 Ace 6
的仓库只会拿到装不上的 zip。标签 `v0.3.2-plu110` 这种“版本-机型”写法新的解析已经会剥掉后缀。

另外记一笔：`cargo test --workspace` 里 `skyward-backup` 的
`a_folder_comes_back_as_it_was_archived` 和 `measuring_and_packing_stop_when_asked`
在本沙盒（proot）里是挂的，未改动的上游树上一样挂，属于环境问题，不是移植引入的。

## 待办

- [ ] wlan0 `Device or resource busy`：进 recovery 自动做一次 down/up
- [ ] 本机 USB 口“两种功能同时在”的实测，决定 `mtp` 的初值
- [ ] 振动周期的来源
- [ ] data 解密完整走一遍（现在 key service 交给手机自己的 init，路径变了，要重测）

---

维护：猫羽bot（记录与合并），移植由名纸酱实测驱动。上游版权与许可同 kirklin/skyward，GPL-3.0-or-later。

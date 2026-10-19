LocalSend QR 0.1.1 — Windows x64 / Ubuntu 24 x64

中文说明
========
这是基于 LocalSend Rust 核心的独立便携工具，无需安装 LocalSend。
解压到可长期保留的目录。手机无需 App，扫码后用系统浏览器下载。

Windows 11 x64
1. 双击 Register-ContextMenu.cmd 注册右键，无需修改 PowerShell 执行策略。
2. 右键一个文件或文件夹 → 显示更多选项 → 通过二维码传送。
3. 终端显示二维码；多网卡时先选择手机可以连接到的 Wi-Fi/以太网地址。
4. 手机扫码，点击“下载文件”。请保持终端窗口开启。
5. 关闭分享用 Ctrl+C。卸载右键双击 Remove-ContextMenu.cmd。
6. 移动便携包后重新运行 Register-ContextMenu.cmd，更新调用路径。
也可以在终端执行 .\localsend-qr.exe context-menu install 注册，或
.\localsend-qr.exe context-menu remove 移除菜单。
可用 context-menu install --core-path 'D:\其他目录\localsend-qr.exe' 指定核心位置。
原 .ps1 注册/移除入口保留供允许 PowerShell 脚本的环境使用。
也可直接使用 EXE 分享文件，无需注册任何右键菜单。

Ubuntu 24 x64（默认 GNOME Files / Nautilus）
1. 在解压目录运行 bash register-context-menu.sh。
2. 右键选中的文件/文件夹 → 脚本 → 通过二维码传送。
3. 移动便携包后重跑注册脚本；可给脚本传入核心程序绝对路径。
4. 移除菜单运行 bash remove-context-menu.sh。
Nautilus 菜单需要 gnome-terminal；命令行分享不需要桌面环境。
在 NTFS/exFAT 等不保留执行权限的盘上，建议先解压到 Ubuntu 用户目录。
Nautilus 通过换行传递选中路径，含换行的文件名请改用命令行。

命令行
======
Windows: .\localsend-qr.exe share -- 'D:\中文 文件.txt'
Ubuntu:  ./localsend-qr share -- '/home/user/中文 文件.txt'
多项:    ./localsend-qr share -- FILE1 FILE2 FOLDER
端口:    ./localsend-qr share --port 0 -- FILE  （自动选择空闲端口）
网卡:    ./localsend-qr share --ip 192.168.1.20 -- FILE
时限:    ./localsend-qr share --expires 0 -- FILE （不自动过期）
配置:    EXE/核心程序旁的 settings.json；命令行参数优先。
默认端口 53318，避免与普通 LocalSend 的 53317 冲突。默认分享 60 分钟。
默认时限到达会终止进行中的下载；大文件可提高时限或用 --expires 0。

行为与网络
==========
单文件直接传输，不复制整个文件到内存。文件夹或多项先生成一个未压缩 ZIP，
保留子目录和空目录，支持 ZIP64；需要系统临时目录有足够空间。
正常停止会删除临时 ZIP；强制结束进程或断电可能留下系统临时文件。
符号链接和特殊文件会报错，避免意外把其他目录内容一起分享。
Windows 右键第一版支持单个文件或文件夹；多选请用命令行。
二维码使用每次随机生成的访问凭证。得到完整链接的人可以下载所选内容。
传输使用 HTTP，未加密，请在可信局域网使用。文件不经云端。
手机和电脑必须可以互相访问；访客 Wi-Fi/AP 隔离、VPN 和防火墙可能阻断。
Windows 第一次可能弹出防火墙提示；允许专用网络。必要时自行以管理员运行
Allow-PrivateNetwork.ps1，仅允许专用网络中本子网设备访问这个程序。
此规则在移动程序后需重跑；不用时可在防火墙中移除 LocalSendQR-Portable-Private。
Ubuntu 若启用了 UFW，可手动只对局域网网段开放使用的 TCP 端口。
浏览器决定保存位置，照片不会自动进入相册；微信内置浏览器可能需要转到系统浏览器。
终端“Download requested”仅表示收到下载请求，不代表手机已完整保存。

许可与来源
==========
项目: https://github.com/KevinChen222/localsend-qr
上游: https://github.com/localsend/localsend
Apache-2.0，保留上游 LICENSE。此 fork 为独立扩展，不是官方发行版。

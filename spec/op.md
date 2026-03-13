# 🚀 PolyInsight Engine 机构级生产环境部署指南

## 🌟 部署架构核心思想 (Deployment Philosophy)

本方案将系统物理拆分为三层，各自采用最顶级的原生方案，实现**零延迟损耗、无限抗并发、免运维数据库**：

1. **重型数据层 (Data)**：ClickHouse Cloud + Serverless Redis（全托管，吃金钱换极速）。
2. **硬核算力层 (Compute)**：Rust 二进制文件 + 裸金属/计算型云主机（纯物理级运行，榨干 CPU）。
3. **边缘呈现层 (Edge UI)**：Next.js + Vercel（全球 CDN 分发，前端永不宕机）。

---

## 🛠️ 第一阶段：基础设施准备 (Infrastructure Setup)

在部署代码前，我们需要先准备好所有 SaaS 级的数据底座。

### 1. 部署时序数据库 (ClickHouse)

* **推荐方案**：注册 **ClickHouse Cloud**（首选）或 Aiven for ClickHouse。
* **操作**：
1. 创建一个新的 Service，选择最靠近你算力服务器的可用区（如 `AWS us-east-1` 或 `ap-northeast-1`）。
2. 获取连接凭证：`CLICKHOUSE_URL`（如 `https://<id>.us-east-1.aws.clickhouse.cloud:8443`）、`USER` 和 `PASSWORD`。
3. **安全配置**：在 ClickHouse 控制台的 IP Allowlist 中，仅放行你 Rust 服务器的公网 IP。



### 2. 部署消息总线 (Redis Streams)

* **推荐方案**：使用 **Upstash** (Serverless Redis) 或在 Rust 服务器同网段（VPC）内开一台 AWS ElastiCache。
* **操作**：获取 `REDIS_URL`（如 `rediss://default:<password>@<endpoint>.upstash.io:33068`）。

### 3. 准备大模型与预言机 API Keys

* **LLM**：准备 OpenAI API Key 或 DeepSeek API Key（推荐 DeepSeek-V3，处理海量资讯性价比极高）。
* **Oracle**：注册 The Odds API、CoinGlass 等并获取 API Keys。

---

## 🦀 第二阶段：Rust 核心引擎部署 (裸金属/EC2 裸跑)

我们不使用 Docker，直接在 Linux 上以 Systemd 守护进程的方式运行编译好的极其紧凑的 Rust 二进制文件。

### 1. 服务器准备

* **推荐机型**：AWS `c7g.large` (ARM 架构更便宜高效) 或 Hetzner 独立物理机。
* **系统**：Ubuntu 22.04 LTS 或 Debian 12。
* **环境初始化**（SSH 登录服务器后）：
```bash
sudo apt update && sudo apt upgrade -y
sudo apt install -y build-essential libssl-dev pkg-config

# 创建专用运行用户，避免使用 root 运行带来安全隐患
sudo useradd -m -s /bin/bash polyinsight

```



### 2. 配置文件就绪

在服务器上创建环境变量文件：

```bash
sudo mkdir -p /opt/polyinsight
sudo nano /opt/polyinsight/.env

```

写入配置：

```ini
# /opt/polyinsight/.env
RUST_LOG=info,polyinsight=debug
SERVER_PORT=8080

CLICKHOUSE_URL=https://...
CLICKHOUSE_USER=default
CLICKHOUSE_PASSWORD=...

REDIS_URL=rediss://...

LLM_API_BASE=https://api.openai.com/v1
LLM_API_KEY=sk-xxxxx

```

### 3. 配置 Systemd 守护进程

创建一个 Linux 服务，让操作系统来管理你的 Rust 引擎（自动重启、开机自启）。

```bash
sudo nano /etc/systemd/system/polyinsight.service

```

写入以下内容：

```ini
[Unit]
Description=PolyInsight Rust Core Engine
After=network.target

[Service]
Type=simple
User=polyinsight
Group=polyinsight
WorkingDirectory=/opt/polyinsight
# 加载环境变量
EnvironmentFile=/opt/polyinsight/.env
# 执行你的 Rust 编译产物
ExecStart=/opt/polyinsight/polyinsight-core

Restart=always
RestartSec=3
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target

```

启用服务：

```bash
sudo systemctl daemon-reload
sudo systemctl enable polyinsight

```

### 4. GitOps 自动化发布 (GitHub Actions)

在你的本地代码库中，配置 `.github/workflows/deploy.yml`，实现**一键推代码，自动部署到物理机**：

```yaml
name: Deploy Rust Core
on:
  push:
    branches: [ "main" ]

jobs:
  build-and-deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Build Rust Release
        run: cargo build --release
        
      - name: Deploy to Server via SCP
        uses: appleboy/scp-action@master
        with:
          host: ${{ secrets.SERVER_IP }}
          username: ${{ secrets.SERVER_USER }}
          key: ${{ secrets.SSH_PRIVATE_KEY }}
          source: "target/release/polyinsight-core"
          target: "/tmp/"
          
      - name: Restart Systemd Service
        uses: appleboy/ssh-action@master
        with:
          host: ${{ secrets.SERVER_IP }}
          username: ${{ secrets.SERVER_USER }}
          key: ${{ secrets.SSH_PRIVATE_KEY }}
          script: |
            sudo mv /tmp/target/release/polyinsight-core /opt/polyinsight/
            sudo chmod +x /opt/polyinsight/polyinsight-core
            sudo systemctl restart polyinsight

```

*💡 效果：每次在本地 `git push`，GitHub 服务器会自动编译好极致优化的 Rust 二进制文件，秒传到你的物理机，并瞬间完成热重启。整个过程如丝般顺滑。*

---

## 🌐 第三阶段：前端数据美学部署 (Next.js @ Vercel)

将面向全球用户的 Dashboard 部署到 Vercel 边缘网络。

### 1. 准备 Vercel 部署

1. 登录 [Vercel](https://vercel.com/)，点击 "Add New Project"。
2. 绑定你的 GitHub 账号，选择 `PolyInsight` 代码库（将 Root Directory 选为前端代码所在的 `frontend` 文件夹）。

### 2. 配置环境变量

在 Vercel 的 Environment Variables 面板中填入：

* `NEXT_PUBLIC_API_URL`：指向你的 Rust 物理机的公网 IP 或绑定的 API 域名（如 `https://api.polyinsight.com`）。

### 3. 一键发布与全球加速

点击 **Deploy**。Vercel 会自动识别 Next.js 项目，构建并将其发布到全球 CDN。

* **绑定域名**：在 Settings -> Domains 中绑定你的极客域名（如 `insight.yourfund.com`），Vercel 会自动免费签发 SSL 证书。

---

## 🔒 第四阶段：安全加固与运维监测 (Day 2 Operations)

### 1. 安全组与防火墙设置 (Firewall)

* **Rust 服务器**：只开放 `SSH (22)` 端口和 `API (8080或443)` 端口。
```bash
sudo ufw allow 22/tcp
sudo ufw allow 8080/tcp
sudo ufw enable

```


* 如果用了 Cloudflare，可以利用 Cloudflare Tunnels 将 Rust 服务器隐藏在内网，避免遭到 DDoS 攻击。

### 2. 日志查看 (Log Monitoring)

由于抛弃了 Docker，查看日志变得更加原生理直气壮。使用 `journalctl` 实时查看 Rust 引擎的微秒级输出：

```bash
# 实时跟踪引擎输出
sudo journalctl -u polyinsight -f -n 100

# 查找所有爆错信息
sudo journalctl -u polyinsight | grep "ERROR"

```

### 3. 反向代理配置 (可选 Nginx / Caddy)

如果你希望 Rust 服务器也提供 HTTPS（Vercel 强制要求调用的 API 必须是 HTTPS），可以在物理机上装一个 Nginx 或 Caddy 作为反向代理转发给 8080 端口，并用 Certbot 自动配置 SSL。
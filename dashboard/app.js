// Real-time Dashboard Application Logic for C2 Simulator

// Application State
let state = {
    agents: new Map(),
    selectedAgentId: null,
    metricsHistory: new Map(), // agentId -> array of { cpu, mem, disk, timestamp }
    chart: null,
    socket: null,
    totalLogsCount: 0
};

// DOM Elements
const elAgentList = document.getElementById('agent-list');
const elWorkspaceTitle = document.getElementById('workspace-title');
const elWorkspaceSubtitle = document.getElementById('workspace-subtitle');
const elOverviewGrid = document.getElementById('overview-grid');
const elAgentWorkspace = document.getElementById('agent-workspace');
const elLogTerminal = document.getElementById('log-terminal');

const elStatTotalAgents = document.getElementById('stat-total-agents');
const elStatOnlineAgents = document.getElementById('stat-online-agents');
const elStatLogCount = document.getElementById('stat-log-count');

const elAgentProfileAvatar = document.getElementById('agent-profile-avatar');
const elAgentProfileHostname = document.getElementById('agent-profile-hostname');
const elAgentProfileId = document.getElementById('agent-profile-id');
const elAgentProfileStatusBadge = document.getElementById('agent-profile-status-badge');
const elAgentProfileOs = document.getElementById('agent-profile-os');
const elAgentProfileLastSeen = document.getElementById('agent-profile-last-seen');

const elTxtCpuUsage = document.getElementById('txt-cpu-usage');
const elTxtMemUsage = document.getElementById('txt-mem-usage');
const elTxtDiskUsage = document.getElementById('txt-disk-usage');
const elProgressCpu = document.getElementById('progress-cpu');
const elProgressMem = document.getElementById('progress-mem');
const elProgressDisk = document.getElementById('progress-disk');

const elBtnRefresh = document.getElementById('btn-refresh');

// Init application
document.addEventListener('DOMContentLoaded', () => {
    initWebSocket();
    setupEventListeners();
    initChart();
    showGlobalOverview();
});

// Setup DOM listeners
function setupEventListeners() {
    elBtnRefresh.addEventListener('click', () => {
        window.location.reload();
    });

    // Logo click returns to global overview
    document.querySelector('.sidebar-header').addEventListener('click', showGlobalOverview);
}

// Show global dashboard overview
function showGlobalOverview() {
    state.selectedAgentId = null;
    elWorkspaceTitle.textContent = "Global Overview";
    elWorkspaceSubtitle.textContent = "Real-time status of all simulated operations";
    
    // Toggle active sidebar item
    document.querySelectorAll('.agent-item').forEach(item => item.classList.remove('active'));
    
    elAgentWorkspace.classList.add('hidden');
    elOverviewGrid.classList.remove('hidden');

    // Refresh general stats
    updateStats();
    
    // Fetch and display global logs
    fetchGlobalLogs();
}

// Fetch global logs from API
async function fetchGlobalLogs() {
    try {
        const res = await fetch('/api/logs');
        if (res.ok) {
            const logs = await res.json();
            if (Array.isArray(logs)) {
                elLogTerminal.innerHTML = '';
                logs.reverse().forEach(log => appendLogToTerminal(log));
            }
        }
    } catch (e) {
        console.error("Failed to fetch global logs", e);
    }
}

// Connect WebSocket to C2 Server
function initWebSocket() {
    const loc = window.location;
    let wsUri = loc.protocol === "https:" ? "wss:" : "ws:";
    
    if (loc.host) {
        wsUri += `//${loc.host}/api/dashboard/ws`;
    } else {
        // Fallback for opening index.html directly from file system
        wsUri = "ws://localhost:3000/api/dashboard/ws";
    }

    console.log("Connecting to WebSocket gateway at:", wsUri);
    const socket = new WebSocket(wsUri);
    state.socket = socket;

    socket.onopen = () => {
        console.log("WebSocket connection established!");
    };

    socket.onmessage = (event) => {
        try {
            const data = JSON.parse(event.data);
            handleServerMessage(data);
        } catch (e) {
            console.error("Error processing WS message:", e);
        }
    };

    socket.onclose = () => {
        console.log("WebSocket connection closed. Retrying in 3 seconds...");
        setTimeout(initWebSocket, 3000);
    };

    socket.onerror = (err) => {
        console.error("WebSocket error:", err);
    };
}

// Route incoming server messages
function handleServerMessage(msg) {
    const payload = msg.payload;
    switch (msg.type) {
        case "InitialAgents":
            state.agents.clear();
            payload.forEach(agent => {
                state.agents.set(agent.id, agent);
            });
            renderAgentList();
            updateStats();
            break;
            
        case "InitialLogs":
            elLogTerminal.innerHTML = '';
            // Logs come from DB sorted by timestamp DESC. Reverse to print chronologically
            payload.reverse().forEach(log => {
                appendLogToTerminal(log);
            });
            state.totalLogsCount += payload.length;
            elStatLogCount.textContent = state.totalLogsCount;
            break;

        case "Log":
            appendLogToTerminal(payload);
            state.totalLogsCount++;
            elStatLogCount.textContent = state.totalLogsCount;
            break;

        case "AgentStatus":
            const agent = state.agents.get(payload.id);
            if (agent) {
                agent.status = payload.status;
                agent.last_seen = payload.last_seen;
                state.agents.set(payload.id, agent);
                updateAgentSidebarItem(agent);
                
                if (state.selectedAgentId === payload.id) {
                    updateAgentDetailsView(agent);
                }
            } else {
                // New agent discovered, request refresh or fetch
                refreshAgents();
            }
            updateStats();
            break;

        case "Metrics":
            const metrics = payload;
            const history = state.metricsHistory.get(metrics.agent_id) || [];
            history.push(metrics);
            // Cap history length
            if (history.length > 50) {
                history.shift();
            }
            state.metricsHistory.set(metrics.agent_id, history);

            if (state.selectedAgentId === metrics.agent_id) {
                // Update progress bars
                updateMetricsVisuals(metrics);
                // Update Chart
                addMetricToChart(metrics);
            }
            break;
    }
}

// Refresh list of agents from Server REST API
async function refreshAgents() {
    try {
        const res = await fetch('/api/agents');
        if (res.ok) {
            const list = await res.json();
            if (Array.isArray(list)) {
                state.agents.clear();
                list.forEach(agent => {
                    state.agents.set(agent.id, agent);
                });
                renderAgentList();
                updateStats();
            }
        }
    } catch (e) {
        console.error("Failed to refresh agents list", e);
    }
}

// Render Sidebar Agents list
function renderAgentList() {
    elAgentList.innerHTML = '';
    if (state.agents.size === 0) {
        elAgentList.innerHTML = '<li class="loading-placeholder">No agents registered</li>';
        return;
    }

    state.agents.forEach(agent => {
        const li = document.createElement('li');
        li.className = `agent-item ${state.selectedAgentId === agent.id ? 'active' : ''}`;
        li.id = `agent-item-${agent.id}`;
        
        const isOnline = agent.status === "Online";
        const statusDotClass = isOnline ? 'pulse-green' : '';
        const osEmoji = agent.os.toLowerCase().includes('windows') ? '💻' : '🐧';

        li.innerHTML = `
            <div class="agent-avatar">${osEmoji}</div>
            <div class="agent-info">
                <h4>${agent.hostname}</h4>
                <div class="agent-meta">
                    <span class="status-dot ${statusDotClass}" style="background-color: ${isOnline ? '#10b981' : '#f43f5e'}"></span>
                    <span>${agent.status}</span>
                </div>
            </div>
        `;

        li.addEventListener('click', () => selectAgent(agent.id));
        elAgentList.appendChild(li);
    });
}

// Update specific agent list item UI (faster than rendering whole list)
function updateAgentSidebarItem(agent) {
    const li = document.getElementById(`agent-item-${agent.id}`);
    if (li) {
        const isOnline = agent.status === "Online";
        const statusDot = li.querySelector('.agent-meta .status-dot');
        const statusText = li.querySelector('.agent-meta span:last-child');
        
        if (statusDot) {
            statusDot.className = `status-dot ${isOnline ? 'pulse-green' : ''}`;
            statusDot.style.backgroundColor = isOnline ? '#10b981' : '#f43f5e';
        }
        if (statusText) {
            statusText.textContent = agent.status;
        }
    } else {
        renderAgentList();
    }
}

// Select an agent and initialize detail workspace
async function selectAgent(agentId) {
    state.selectedAgentId = agentId;
    
    // Toggle active classes in sidebar
    document.querySelectorAll('.agent-item').forEach(item => {
        item.classList.remove('active');
    });
    const activeItem = document.getElementById(`agent-item-${agentId}`);
    if (activeItem) {
        activeItem.classList.add('active');
    }

    const agent = state.agents.get(agentId);
    if (!agent) return;

    // Transition panels
    elOverviewGrid.classList.add('hidden');
    elAgentWorkspace.classList.remove('hidden');

    // Update Header titles
    elWorkspaceTitle.textContent = `Agent: ${agent.hostname}`;
    elWorkspaceSubtitle.textContent = `Telemetry and configurations for agent ID: ${agent.id}`;

    updateAgentDetailsView(agent);
    
    // Reset metrics visuals
    updateMetricsVisuals({ cpu_usage: 0, memory_usage: 0, disk_usage: 0 });

    // Fetch historical data for chart initialization
    await fetchAgentMetricsHistory(agentId);
    await fetchAgentLogs(agentId);
}

// Update profile card details
function updateAgentDetailsView(agent) {
    elAgentProfileHostname.textContent = agent.hostname;
    elAgentProfileId.textContent = `ID: ${agent.id}`;
    
    const isOnline = agent.status === "Online";
    elAgentProfileStatusBadge.textContent = agent.status;
    elAgentProfileStatusBadge.className = `badge ${isOnline ? 'online' : 'offline'}`;
    
    elAgentProfileOs.textContent = agent.os;
    elAgentProfileLastSeen.textContent = formatDate(agent.last_seen);
}

// Update metrics visual elements
function updateMetricsVisuals(metrics) {
    elTxtCpuUsage.textContent = `${metrics.cpu_usage.toFixed(1)}%`;
    elTxtMemUsage.textContent = `${metrics.memory_usage.toFixed(1)}%`;
    elTxtDiskUsage.textContent = `${metrics.disk_usage.toFixed(1)}%`;

    elProgressCpu.style.width = `${metrics.cpu_usage}%`;
    elProgressMem.style.width = `${metrics.memory_usage}%`;
    elProgressDisk.style.width = `${metrics.disk_usage}%`;
}

// Get recent logs of specific agent
async function fetchAgentLogs(agentId) {
    try {
        const res = await fetch(`/api/agents/${agentId}/logs`);
        if (res.ok) {
            const logs = await res.json();
            if (Array.isArray(logs)) {
                elLogTerminal.innerHTML = '';
                logs.reverse().forEach(log => appendLogToTerminal(log));
            }
        }
    } catch (e) {
        console.error("Failed to fetch agent logs", e);
    }
}

// Get metrics history from server API
async function fetchAgentMetricsHistory(agentId) {
    try {
        const res = await fetch(`/api/agents/${agentId}/metrics`);
        if (res.ok) {
            const metricsList = await res.json();
            if (Array.isArray(metricsList)) {
                state.metricsHistory.set(agentId, metricsList);
                initializeChartData(metricsList);
                if (metricsList.length > 0) {
                    updateMetricsVisuals(metricsList[metricsList.length - 1]);
                }
            }
        }
    } catch (e) {
        console.error("Failed to fetch metrics history", e);
    }
}

// Render dynamic log lines
function appendLogToTerminal(log) {
    const isGlobal = state.selectedAgentId === null;
    // If agent is selected, only show logs matching that agent or general Server logs
    if (!isGlobal && log.agent_id && log.agent_id !== state.selectedAgentId) {
        return; 
    }

    const line = document.createElement('div');
    line.className = 'log-line';
    
    const formattedTime = formatDate(log.timestamp).split(' ')[1] || log.timestamp;
    const sourceClass = log.source.toLowerCase() === 'server' ? 'server' : 'agent';
    const levelClass = `level-${log.level.toLowerCase()}`;
    
    const agentLabel = log.agent_id ? `[Agent: ${log.agent_id.substring(0, 8)}] ` : '';

    line.innerHTML = `
        <span class="log-time">[${formattedTime}]</span>
        <span class="log-level ${levelClass}">${log.level}</span>
        <span class="log-source ${sourceClass}">${log.source}</span>
        <span class="log-msg">${agentLabel}${log.message}</span>
    `;

    elLogTerminal.appendChild(line);
    // Autoscroll terminal
    elLogTerminal.scrollTop = elLogTerminal.scrollHeight;
}

// Update Top Statistics Boxes
function updateStats() {
    elStatTotalAgents.textContent = state.agents.size;
    let online = 0;
    state.agents.forEach(agent => {
        if (agent.status === "Online") online++;
    });
    elStatOnlineAgents.textContent = online;
}

// Initialize Chart.js
function initChart() {
    const ctx = document.getElementById('metricsChart').getContext('2d');
    
    state.chart = new Chart(ctx, {
        type: 'line',
        data: {
            labels: [],
            datasets: [
                {
                    label: 'CPU Usage (%)',
                    data: [],
                    borderColor: '#6366f1',
                    backgroundColor: 'rgba(99, 102, 241, 0.1)',
                    borderWidth: 2,
                    tension: 0.3,
                    fill: true
                },
                {
                    label: 'Memory Usage (%)',
                    data: [],
                    borderColor: '#3b82f6',
                    backgroundColor: 'rgba(59, 130, 246, 0.1)',
                    borderWidth: 2,
                    tension: 0.3,
                    fill: true
                }
            ]
        },
        options: {
            responsive: true,
            maintainAspectRatio: false,
            plugins: {
                legend: {
                    labels: { color: '#94a3b8', font: { family: 'Inter' } }
                }
            },
            scales: {
                x: {
                    grid: { color: 'rgba(255,255,255,0.05)' },
                    ticks: { color: '#64748b', font: { family: 'Inter', size: 9 } }
                },
                y: {
                    min: 0,
                    max: 100,
                    grid: { color: 'rgba(255,255,255,0.05)' },
                    ticks: { color: '#64748b', font: { family: 'Inter' } }
                }
            }
        }
    });
}

// Load historical list into Chart
function initializeChartData(metricsList) {
    if (!state.chart) return;
    
    const labels = [];
    const cpuData = [];
    const memData = [];

    metricsList.forEach(m => {
        const time = m.timestamp.split('T')[1] ? m.timestamp.split('T')[1].substring(0, 8) : m.timestamp;
        labels.push(time);
        cpuData.push(m.cpu_usage);
        memData.push(m.memory_usage);
    });

    state.chart.data.labels = labels;
    state.chart.data.datasets[0].data = cpuData;
    state.chart.data.datasets[1].data = memData;
    state.chart.update();
}

// Push a single metrics frame to Chart
function addMetricToChart(metric) {
    if (!state.chart) return;
    
    const time = metric.timestamp.split('T')[1] ? metric.timestamp.split('T')[1].substring(0, 8) : metric.timestamp;
    
    state.chart.data.labels.push(time);
    state.chart.data.datasets[0].data.push(metric.cpu_usage);
    state.chart.data.datasets[1].data.push(metric.memory_usage);

    // Limit points on chart
    if (state.chart.data.labels.length > 30) {
        state.chart.data.labels.shift();
        state.chart.data.datasets[0].data.shift();
        state.chart.data.datasets[1].data.shift();
    }
    
    state.chart.update();
}

// Formatting date utility
function formatDate(isoStr) {
    if (!isoStr) return '-';
    try {
        const d = new Date(isoStr);
        const yyyy = d.getFullYear();
        const mm = String(d.getMonth() + 1).padStart(2, '0');
        const dd = String(d.getDate()).padStart(2, '0');
        const hh = String(d.getHours()).padStart(2, '0');
        const min = String(d.getMinutes()).padStart(2, '0');
        const ss = String(d.getSeconds()).padStart(2, '0');
        return `${yyyy}-${mm}-${dd} ${hh}:${min}:${ss}`;
    } catch (e) {
        return isoStr;
    }
}

import React from 'react';
import { Line } from 'react-chartjs-2';
import {
  Chart as ChartJS,
  CategoryScale, LinearScale,
  PointElement, LineElement,
  Tooltip, Legend
} from 'chart.js';

ChartJS.register(CategoryScale, LinearScale, PointElement, LineElement, Tooltip, Legend);

export function MetricsChart({ data }) {
  if (!data || data.length === 0) return null;

  const labels = data.map(m => {
    const t = m.timestamp.split('T')[1] || m.timestamp;
    return t.substring(0, 8);
  });

  const chartData = {
    labels,
    datasets: [
      {
        label: 'cpu',
        data: data.map(m => m.cpu_usage),
        borderColor: '#69a',
        backgroundColor: 'rgba(102,153,170,0.08)',
        borderWidth: 1,
        tension: 0.2,
        fill: true,
        pointRadius: 0,
      },
      {
        label: 'mem',
        data: data.map(m => m.memory_usage),
        borderColor: '#b93',
        backgroundColor: 'rgba(187,153,51,0.08)',
        borderWidth: 1,
        tension: 0.2,
        fill: true,
        pointRadius: 0,
      },
    ],
  };

  const options = {
    responsive: true,
    maintainAspectRatio: false,
    animation: { duration: 0 },
    plugins: {
      legend: {
        labels: { color: '#666', font: { family: "'JetBrains Mono', monospace", size: 10 }, boxWidth: 10 },
      },
    },
    scales: {
      x: {
        grid: { color: '#222' },
        ticks: { color: '#555', font: { family: "'JetBrains Mono', monospace", size: 9 }, maxTicksLimit: 8 },
      },
      y: {
        min: 0, max: 100,
        grid: { color: '#222' },
        ticks: { color: '#555', font: { family: "'JetBrains Mono', monospace", size: 9 } },
      },
    },
  };

  return (
    <div className="chart-area">
      <div style={{ fontSize: '10px', color: '#555', textTransform: 'uppercase', letterSpacing: '1px', marginBottom: '6px' }}>
        performance history
      </div>
      <div className="chart-wrap">
        <Line data={chartData} options={options} />
      </div>
    </div>
  );
}

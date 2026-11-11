import * as echarts from 'echarts';

export interface TelemetryChartManager {
  chart: echarts.ECharts;
  updateData: (timestamps: string[], values: number[]) => void;
  resize: () => void;
}

/**
 * Initializes Apache ECharts telemetry line chart matching dark-first terminal theme.
 */
export function initTelemetryChart(containerId: string): TelemetryChartManager {
  const container = document.getElementById(containerId);
  if (!container) {
    throw new Error(`Element #${containerId} not found`);
  }

  const chart = echarts.init(container, 'dark', {
    renderer: 'canvas',
  });

  const timestamps = ['12:00', '12:10', '12:20', '12:30', '12:40', '12:50', '13:00'];
  const values = [2.1, 2.4, 2.0, 3.2, 2.8, 3.0, 2.7];

  const option: echarts.EChartsOption = {
    backgroundColor: '#161b22',
    tooltip: {
      trigger: 'axis',
      backgroundColor: '#21262d',
      borderColor: '#30363d',
      textStyle: {
        color: '#c9d1d9',
        fontFamily: 'monospace',
        fontSize: 11,
      },
    },
    grid: {
      top: 24,
      right: 16,
      bottom: 24,
      left: 36,
      containLabel: true,
    },
    xAxis: {
      type: 'category',
      data: timestamps,
      axisLine: { lineStyle: { color: '#30363d' } },
      axisLabel: { color: '#8b949e', fontSize: 10, fontFamily: 'monospace' },
    },
    yAxis: {
      type: 'value',
      name: 'Kp Index',
      nameTextStyle: { color: '#8b949e', fontSize: 9, fontFamily: 'monospace' },
      axisLine: { lineStyle: { color: '#30363d' } },
      splitLine: { lineStyle: { color: '#21262d' } },
      axisLabel: { color: '#8b949e', fontSize: 10, fontFamily: 'monospace' },
    },
    series: [
      {
        name: 'Space Weather Kp',
        type: 'line',
        smooth: true,
        data: values,
        itemStyle: { color: '#58a6ff' },
        lineStyle: { width: 2, color: '#58a6ff' },
        areaStyle: {
          color: new echarts.graphic.LinearGradient(0, 0, 0, 1, [
            { offset: 0, color: 'rgba(88, 166, 255, 0.35)' },
            { offset: 1, color: 'rgba(88, 166, 255, 0.0)' },
          ]),
        },
      },
    ],
  };

  chart.setOption(option);

  window.addEventListener('resize', () => {
    chart.resize();
  });

  return {
    chart,
    updateData: (newTimestamps: string[], newValues: number[]) => {
      chart.setOption({
        xAxis: { data: newTimestamps },
        series: [{ data: newValues }],
      });
    },
    resize: () => chart.resize(),
  };
}

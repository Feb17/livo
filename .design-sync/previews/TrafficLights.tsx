import { TrafficLights } from '@livo/design';

export const Default = () => (
  <div style={{ display: 'flex', gap: 32 }}>
    <div style={{ background: 'var(--sidebar)', borderRadius: 8, paddingRight: 16 }}><TrafficLights /></div>
    <div style={{ background: 'var(--sidebar)', borderRadius: 8, paddingRight: 16 }}><TrafficLights fullscreenDisabled /></div>
  </div>
);

import { SearchField } from '@livo/design';

export const Empty = () => (<div style={{ width: 360 }}><SearchField /></div>);
export const WithQuery = () => (<div style={{ width: 360 }}><SearchField value="原神" focused /></div>);

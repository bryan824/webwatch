import { render } from 'solid-js/web';
import { App } from './App';
import './styles/reset.css';
import './styles/instrument.css';

const root = document.getElementById('app');
if (root) render(() => <App />, root);

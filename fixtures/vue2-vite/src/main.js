import Vue from 'vue';
import App from './App.vue';

Vue.config.productionTip = false;
const app = new Vue(App).$mount('#app');

// Fixture-only access for verifying prop updates and destroyed instances.
window.vue2Fixture = { app, version: Vue.version };
if (import.meta.hot) import.meta.hot.dispose(() => app.$destroy());

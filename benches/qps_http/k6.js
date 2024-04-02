import http from "k6/http";

export const options = {
  discardResponseBodies: true,

  scenarios: {
    contacts: {
      executor: 'ramping-arrival-rate',

      // Start iterations per `timeUnit`
      startRate: 500,

      // Start `startRate` iterations per minute
      timeUnit: '10s',

      // Pre-allocate necessary VUs.
      preAllocatedVUs: 50,

      stages: [
        { target: 1000, duration: '10s' },
        { target: 2000, duration: '10s' },
        { target: 5000, duration: '10s' },
        { target: 10000, duration: '10s' },
        { target: 20000, duration: '10s' },
        { target: 50000, duration: '10s' },
      ],
    },
  },
};
const params = {
  timeout: '3s'
}
export default function () {
  http.get('http://127.0.0.1:3001', params);
}
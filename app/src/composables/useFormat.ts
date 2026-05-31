export const formatNum = (val?: number) => val?.toFixed(2) || '---';
export const formatPrice = (val?: number) => val?.toLocaleString(undefined, { minimumFractionDigits: 1 }) || '---';
export const formatPercent = (val?: number) => `${val?.toFixed(2) || '0.00'}%`;

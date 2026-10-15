// Contratos entre modulos (escritos antes del codigo, aprobados en el walkthrough).
export interface Tramo { desde: number; fijo: number; tasa: number }
export interface Parametros {
  topeBaseImponible: number;        // ANSES, base imponible maxima mensual de aportes
  gananciaNoImponible: number;      // ARCA art. 30 inc. a), anual
  deduccionEspecial: number;        // ARCA art. 30 inc. c) ap. 2, anual
  aportes: { jubilacion: number; pami: number; obraSocial: number };
  escala: Tramo[];                  // ARCA art. 94, anual
  vigencia: { desde: string; hasta: string };   // AAAA-MM
  fuentes: string[];                // URLs https de las fuentes oficiales
}
export interface Desglose { bruto: number; jubilacion: number; pami: number; obraSocial: number; ganancias: number; neto: number }
export type CalcularNeto = (bruto: number, p: Parametros) => Desglose;

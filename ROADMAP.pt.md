# Roadmap do ZedSuite

Esta página lista o que está previsto, o que os utilizadores pediram e o que não está previsto.

Para pedir uma funcionalidade ou reportar um bug: abra uma issue no [GitHub](https://github.com/LeZed97/ZedSuite/issues), ou contacte-me nas minhas redes sociais: [linktr.ee/zedperf](https://linktr.ee/zedperf). Todos os relatos são lidos.

## Previsto

- Comparação dos mapas de duas versões de um projeto na janela Comparar, que hoje faz a comparação binária: o mesmo mapa das duas versões lado a lado, com as diferenças realçadas.
- Vista 2D ao estilo WinOLS para as matrizes completas: uma curva por linha, linha selecionada em destaque.
- Um switch de inversão do N75 na EDC15VM, para os carros que passaram de um turbo com wastegate para um VNT ou o inverso: o bloco que o comanda está localizado na maioria dos ficheiros do banco de testes, o switch em si ainda não está feito.
- EDC15P dos primeiros PD (1999-2002, 038906019A / 019AJ): a deteção dos mapas está feita e a tabela de DTC é lida nos 019AJ; o checksum ainda não é suportado, a tabela de DTC dos 019A usa ainda outra disposição, e o mapa MAP linearisation não é encontrado nos 019A.
- Melhor deteção EDC15VM: alguns ficheiros ainda não estão totalmente cobertos, o 2.5 V6 em particular, e os mapas N146 e N75 da geração 012K / 012AP. As correções estão feitas para os números de software enviados pelos utilizadores (SOI único e switch MAP/MAF do 012M na 1.1.7).
- EDC15VM: verificar no carro que os mapas SVRL estão mesmo ativos quando o detetor os encontra.
- EDC16U31: melhor deteção, ainda faltam alguns ficheiros EDC16U31 no banco de testes para terminar como deve ser (a família 12x12 em 0x1D7xxx continua sem nome).
- EDC16U1: identificação dos ficheiros Touareg V10, onde hoje só um dos dois números de ECU é encontrado (seis ficheiros no banco de testes).
- Compatibilidade XDF (ficheiros de definição TunerPro, para ler e escrever listas de mapas nesse formato). Previsto para mais tarde, quando tiver tempo para me debruçar sobre isso.

## Pedido pelos utilizadores, em análise

- Anular com Ctrl+Z no editor.
- Importação e exportação CSV (o mappack JSON para o WinOLS já existe), e importação DAMOS.
- Mapas favoritos, para aceder depressa aos que mais se editam.
- Uma versão de referência diferente da Ori para o "valor original" e a comparação.
- Editar os mapas diretamente na vista 3D.
- Ajustar a estimativa de potência para outros bicos além dos Firad, como Recambo ou DSSR.
- Editar duas versões do mesmo projeto lado a lado.
- Mais funções na janela Propriedades dos mapas.
- Realçar todos os valores diferentes de origem nas janelas dos mapas.

## Não previsto por agora

- Novas famílias de ECU feitas por mim (EDC15/EDC16 BMW e PSA, EDC17, etc.). Um detetor leva no mínimo dois a três meses e um grande corpo de ficheiros originais e mappacks para ficar certo, e eu mantenho o ZedSuite no meu tempo livre: um trabalho dessa dimensão não é algo que possa oferecer de graça. Uma família pode sempre vir de uma contribuição que cumpra a fasquia do [CONTRIBUTING.md](CONTRIBUTING.md).
- Soluções automáticas (EGR off, DPF off, preparações num clique). O ZedSuite continua a ser uma ferramenta para aprender e compreender o ficheiro, no espírito do EDCSuite.
- Uma rotina de comutação de mapas (multimap) injetada na ECU: faça-o no WinOLS com as rotinas que circulam para EDC15, depois importe o ficheiro como versão, o codeblock adicionado e os seus mapas aparecem desde a 1.1.6.
- Mais dados de referência de ECU (marcas, motores) para o ecrã de importação.
- Compatibilidade com o Windows 7.

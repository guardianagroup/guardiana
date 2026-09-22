# Guardiana

*Idiomas: [Español](README.md) · [English](README.en.md) · **Português***

Programa para Windows, Linux e macOS que transforma o computador no **guardião DNS da casa**:
primeiro dele mesmo, depois dos celulares, da TV e de tudo o que usa o Wi‑Fi, sem instalar nada
neles. Vê com quais serviços cada aparelho tenta falar, classifica, explica em uma frase, anota
em um extrato encadeado por hash e corta apenas o que o usuário decidir.

**Tudo acontece na casa.** Sem conta, sem servidor nosso, zero telemetria. As únicas conexões de
saída são as que o usuário provoca (ativar licença, conferir versão, atualizar listas) e cada uma
fica anotada no próprio extrato.

- O que ele não faz, dito com essas palavras: [docs/WHAT_IT_DOES_NOT_DO.md](docs/WHAT_IT_DOES_NOT_DO.md)
- Modelo de ameaças: [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md)
- Como conferir que o instalado é o publicado: [docs/VERIFY.md](docs/VERIFY.md)
- Modo Casa (celulares sem app): [docs/HOGAR.md](docs/HOGAR.md) e os guias de
  [roteador](docs/guias/router.md), [iPhone](docs/guias/iphone.md) e [Android](docs/guias/android.md)
- Listas usadas e com qual licença: [docs/LISTS.md](docs/LISTS.md)
- Guia do beta fechado: [docs/BETA.md](docs/BETA.md)
- Decisões tomadas e testes feitos: [docs/DECISIONES.md](docs/DECISIONES.md), [docs/PRUEBAS.md](docs/PRUEBAS.md)

## Instalar

| Sistema | Pacote | Como |
|---|---|---|
| Windows 10/11 (64 bits) | `guardiana-<versão>-windows-x64-pt.msi` (em espanhol, `…-x64.msi`; em inglês, `…-x64-en.msi`) | Clique duplo. Instala em Arquivos de Programas e registra o serviço. |
| Debian, Ubuntu e derivados | `guardiana_<versão>_amd64.deb` | `sudo apt install ./guardiana_<versão>_amd64.deb` |
| Outro Linux com systemd | `guardiana-<versão>-linux-x86_64.tar.gz` | Descompactar e `sudo ./instalar.sh` |

Instalar **não muda o DNS do sistema**: isso se faz pelo painel, com consentimento, e se desfaz
no mesmo lugar. Desinstalar desliga o Modo Casa, tira a regra do firewall e restaura o DNS
exatamente como estava.

Antes de instalar, compare a impressão SHA‑256 do arquivo com a de `SHA256SUMS` e com a linha
correspondente de [`ledger.jsonl`](ledger.jsonl), o registro público que sai antes do download.
Depois de instalar, `guardiana verify` confere isso no seu computador.

## Usar

```
guardiana panel          abre o painel no navegador (a única interface)
guardiana verify         impressão, assinatura, serviço, DNS do sistema, portas, listas, cadeia
guardiana dns --status   para onde aponta o DNS do sistema
guardiana hogar status   estado do Modo Casa
guardiana ledger --check confere a cadeia do extrato
guardiana export         exporta o extrato em CSV ou JSON
```

Nada é bloqueado sem decisão do usuário, sempre com "desfazer" à vista: um nome concreto é
cortado desde o primeiro minuto, e os cortes largos —por categoria, para a casa inteira ou o modo
que corta tudo o que não foi declarado— esperam a Guardiana olhar esse aparelho por 24 horas. Nenhum sinal é um veredito; a interface nunca diz "malicioso".

## Compilar

Rust estável, edição 2021, `Cargo.lock` no repositório.

```
cargo build --workspace --locked
cargo test --workspace --locked
```

Compilação reproduzível em um contêiner fixado por digest: `build/repro.sh` (veja
[docs/VERIFY.md](docs/VERIFY.md)). Pacotes: `build/package.sh` (Linux) e `build/msi.ps1`
(Windows). Estrutura do código e regras de trabalho: [CLAUDE.md](CLAUDE.md) e
[docs/BRIEF.md](docs/BRIEF.md).

## Licença

GPL‑3.0‑or‑later. As listas de terceiros mantêm a sua licença (EasyPrivacy: GPL‑3.0 / CC BY‑SA
3.0; lista de Peter Lowe: uso livre com atribuição). Detalhe em `docs/LISTS.md`. As tipografias que o
painel serve a partir do próprio computador (IBM Plex e Unbounded) vão com a sua licença OFL 1.1
ao lado, em [`crates/panel/static/fonts`](crates/panel/static/fonts/LEEME.md).
